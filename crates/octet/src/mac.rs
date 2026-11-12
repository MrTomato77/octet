//! The macOS side: the same answers net.rs and adapters.rs give on Windows. An "adapter" is a
//! network service (what System Settings lists: "Wi-Fi", "USB 10/100/1000 LAN"), read from
//! SystemConfiguration's preferences (what is set) and dynamic store (what is live).
//! Ping, TCP and the ARP table use BSD sockets directly; none of the reading needs root.
//! Changes run networksetup in a second copy of Octet, started as root through the system's
//! administrator prompt (see `sudo`).

use crate::adapters::Raw;
use crate::net::Tcp;
use objc2_core_foundation::{CFArray, CFBoolean, CFData, CFDate, CFDictionary, CFNumber, CFRetained, CFString, CFType};
use objc2_system_configuration::{SCDynamicStore, SCNetworkInterface, SCPreferences};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::time::{Duration, Instant, UNIX_EPOCH};

// ---- property lists as JSON, so the rest is plain Rust ----

/// A CF property list as JSON: dates become ms since 1970, data a list of bytes.
fn json(v: &CFType) -> Value {
    if let Some(s) = v.downcast_ref::<CFString>() {
        return s.to_string().into();
    }
    if let Some(b) = v.downcast_ref::<CFBoolean>() {
        return b.as_bool().into();
    }
    if let Some(n) = v.downcast_ref::<CFNumber>() {
        return if n.is_float_type() { n.as_f64().map(Value::from) } else { n.as_i64().map(Value::from) }.unwrap_or_default();
    }
    if let Some(d) = v.downcast_ref::<CFDate>() {
        return d.to_system_time().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(Value::Null, |t| (t.as_millis() as u64).into());
    }
    if let Some(d) = v.downcast_ref::<CFData>() {
        return d.to_vec().into();
    }
    if let Some(a) = v.downcast_ref::<CFArray>() {
        // SAFETY: a property list array only holds CF objects.
        let a: &CFArray<CFType> = unsafe { &*(a as *const CFArray).cast() };
        return a.iter().map(|x| json(&x)).collect();
    }
    if let Some(d) = v.downcast_ref::<CFDictionary>() {
        // SAFETY: as above; keys that aren't strings are skipped.
        let d: &CFDictionary<CFType, CFType> = unsafe { &*(d as *const CFDictionary).cast() };
        let (keys, values) = d.to_vecs();
        return Value::Object(keys.iter().zip(&values).filter_map(|(k, v)| Some((k.downcast_ref::<CFString>()?.to_string(), json(v)))).collect());
    }
    Value::Null
}

fn store() -> Option<CFRetained<SCDynamicStore>> {
    unsafe { SCDynamicStore::new(None, &CFString::from_static_str("Octet"), None, std::ptr::null_mut()) }
}

/// Every dynamic store key matching one of `patterns` (regex), with its value.
fn live(patterns: &[&str]) -> Map<String, Value> {
    let Some(s) = store() else { return Map::new() };
    let pats: Vec<_> = patterns.iter().map(|p| CFString::from_str(p)).collect();
    let pats = CFArray::from_retained_objects(&pats);
    let pats: &CFArray = pats.as_ref();
    match unsafe { SCDynamicStore::multiple(Some(&s), None, Some(pats)) }.map(|d| json(&d)) {
        Some(Value::Object(m)) => m,
        _ => Map::new(),
    }
}

/// Top-level values of the network preferences (/Library/Preferences/SystemConfiguration/preferences.plist).
fn prefs(keys: &[&str]) -> Map<String, Value> {
    let Some(p) = SCPreferences::new(None, &CFString::from_static_str("Octet"), None) else { return Map::new() };
    keys.iter()
        .filter_map(|k| {
            let v = SCPreferences::value(&p, &CFString::from_str(k))?;
            Some((k.to_string(), json(&v)))
        })
        .collect()
}

// ---- the adapter list ----

fn ips(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().filter_map(Value::as_str).filter(|s| s.parse::<Ipv4Addr>().is_ok()).map(String::from).collect()
}

/// The service Octet adds for one more address on `primary`'s port (macOS keeps one address per service;
/// System Settings' Duplicate Service is the same thing). Its address in the name, so it reads as what it is there.
pub(crate) fn extra_name(primary: &str, address: &str) -> String {
    format!("{primary} + {address}")
}

/// The network services of the current location, in System Settings' order, as Windows-shaped rows.
/// Services macOS hides (HiddenConfiguration) and ones without a network device (VPNs, PPP) are left out.
/// One with IPv4 turned off is kept, with no address (Release turns it off; Renew, or DHCP on Apply, back on).
/// An extra-address service (`extra_name`) becomes one more address and gateway of the service it was made beside.
/// `names`: each service's name as macOS gives it (`service_names`); a service missing there falls back to what is stored.
pub(crate) fn services(prefs: &Map<String, Value>, live: &Map<String, Value>, names: &HashMap<String, String>) -> Vec<(Raw, bool)> {
    let set = prefs.get("CurrentSet").and_then(Value::as_str).and_then(|s| s.strip_prefix("/Sets/")).unwrap_or_default();
    let set = &prefs["Sets"][set];
    let mut ids: Vec<String> = set["Network"]["Global"]["IPv4"]["ServiceOrder"].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from).collect();
    // Services of the set that the order leaves out still count, after the ordered ones.
    for id in set["Network"]["Service"].as_object().into_iter().flat_map(|m| m.keys()) {
        if !ids.contains(id) {
            ids.push(id.clone());
        }
    }
    let state = |k: String| live.get(&k).unwrap_or(&Value::Null);
    let mut out = vec![];
    for id in ids {
        let s = &prefs["NetworkServices"][&id];
        let (iface, v4) = (&s["Interface"], &s["IPv4"]);
        let Some(dev) = iface["DeviceName"].as_str() else { continue };
        let method = v4["ConfigMethod"].as_str().unwrap_or("");
        // IPv4 Off: networksetup -setv4off (and System Settings' Off) keep the method and mark the IPv4 entity
        // __INACTIVE__; no ConfigMethod at all is off too. Link-local-only and other methods have nothing to edit.
        let off = matches!(method, "" | "Off") || v4["__INACTIVE__"].as_bool() == Some(true) || v4["__INACTIVE__"].as_i64().is_some_and(|n| n != 0);
        if s.is_null() || iface["HiddenConfiguration"].as_bool() == Some(true) || matches!(iface["Type"].as_str(), Some("PPP" | "VPN" | "IPSec"))
            || !(off || matches!(method, "DHCP" | "BOOTP" | "Manual" | "INFORM")) {
            continue;
        }
        let inactive = s["__INACTIVE__"].as_i64().is_some_and(|n| n != 0) || s["__INACTIVE__"].as_bool() == Some(true);
        let ipv4 = state(format!("State:/Network/Service/{id}/IPv4"));
        let dns = state(format!("State:/Network/Service/{id}/DNS"));
        let dhcp = state(format!("State:/Network/Service/{id}/DHCP"));
        // No Link key (a bridge with nothing in it): connected when it has an address.
        let up = state(format!("State:/Network/Interface/{dev}/Link"))["Active"].as_bool().unwrap_or(!ipv4.is_null());
        let auto = off || matches!(method, "DHCP" | "BOOTP");
        let static_dns = !ips(&s["DNS"]["ServerAddresses"]).is_empty();
        let name = names.get(&id).cloned()
            .unwrap_or_else(|| s["UserDefinedName"].as_str().or(iface["UserDefinedName"].as_str()).unwrap_or(dev).to_string());
        // One of Octet's extra addresses: a row of the earlier service on this port, not an adapter of its own.
        // The name must still match its address, so removing it (by that name) finds it.
        let (set_ip, set_mask) = (ips(&v4["Addresses"]), ips(&v4["SubnetMasks"]));
        if method == "Manual" && let (Some(ip), Some(mask)) = (set_ip.first(), set_mask.first())
            && let Some((p, _)) = out.iter_mut().find(|(p, _): &&mut (Raw, bool)| p.description == dev && name == extra_name(&p.name, ip)) {
            if !inactive {
                p.ip.push(ip.clone());
                p.subnet.push(mask.clone());
                if let Some(r) = v4["Router"].as_str().filter(|r| r.parse::<Ipv4Addr>().is_ok() && !p.gateway.iter().any(|g| g == r)) {
                    p.gateway.push(r.to_string());
                }
            }
            continue;
        }
        let mut raw = Raw {
            id: id.clone(),
            index: index_of(dev),
            name,
            description: dev.to_string(),
            status: if inactive { "Disabled" } else if up { "Up" } else { "Disconnected" }.into(),
            dhcp: auto,
            static_dns,
            ipv4_off: off,
            ..Default::default()
        };
        let wifi_card = iface["Type"].as_str() == Some("IEEE80211") || iface["Hardware"].as_str() == Some("AirPort");
        if !inactive && !off {
            // Live values when the service is up; a static service that isn't keeps what it is set to.
            let src = if ipv4.is_null() && !auto { v4 } else { ipv4 };
            (raw.ip, raw.subnet) = (ips(&src["Addresses"]), ips(&src["SubnetMasks"]));
            raw.gateway = src["Router"].as_str().filter(|r| r.parse::<Ipv4Addr>().is_ok()).map(|r| vec![r.to_string()]).unwrap_or_default();
            // Servers set by hand are the ones macOS uses; the service's live DNS keeps what DHCP offered all the same
            // (the router, say), so it counts only when none are set.
            raw.dns = if static_dns || dns.is_null() { ips(&s["DNS"]["ServerAddresses"]) } else { ips(&dns["ServerAddresses"]) };
            raw.suffix = dns["DomainName"].as_str().unwrap_or_default().to_string();
            if auto && up {
                // Option 54, the server identifier, as 4 bytes.
                let server: Vec<u8> = dhcp["Option_54"].as_array().into_iter().flatten().filter_map(|b| b.as_u64().map(|b| b as u8)).collect();
                if let Ok(b) = <[u8; 4]>::try_from(server.as_slice()) {
                    raw.dhcp_server = Ipv4Addr::from(b).to_string();
                }
                raw.lease_got = dhcp["LeaseStartTime"].as_u64().unwrap_or(0);
                raw.lease_ends = dhcp["LeaseExpirationTime"].as_u64().unwrap_or(0);
            }
        }
        out.push((raw, wifi_card));
    }
    out
}

/// Each service with whether its device is a Wi-Fi card.
pub(crate) fn read() -> Result<Vec<(Raw, bool)>, String> {
    let p = prefs(&["CurrentSet", "Sets", "NetworkServices"]);
    if p.is_empty() {
        return Err("Could not read the network settings".into());
    }
    let l = live(&["State:/Network/Service/[^/]+/(IPv4|DNS|DHCP)", "State:/Network/Interface/[^/]+/Link"]);
    Ok(services(&p, &l, &service_names()))
}

/// Each service's name by service ID, as SCNetworkServiceGetName gives it: what System Settings shows and what
/// networksetup takes. Not always the stored UserDefinedName: a stored default ("iPhone") reads as the port's
/// current name ("iPhone USB"), and networksetup knows the service only by that.
fn service_names() -> HashMap<String, String> {
    use objc2_system_configuration::SCNetworkService;
    let Some(p) = SCPreferences::new(None, &CFString::from_static_str("Octet"), None) else { return HashMap::new() };
    let Some(all) = SCNetworkService::all(&p) else { return HashMap::new() };
    // SAFETY: SCNetworkServiceCopyAll returns an array of SCNetworkService.
    let all: &CFArray<SCNetworkService> = unsafe { &*(&*all as *const CFArray).cast() };
    all.iter().filter_map(|s| Some((s.service_id()?.to_string(), s.name()?.to_string()))).collect()
}

/// The card's own name for each BSD device ("en5" -> "USB 10/100/1000 LAN").
pub(crate) fn display_names() -> HashMap<String, String> {
    let all = SCNetworkInterface::all();
    // SAFETY: SCNetworkInterfaceCopyAll returns an array of SCNetworkInterface.
    let all: &CFArray<SCNetworkInterface> = unsafe { &*(&*all as *const CFArray).cast() };
    all.iter().filter_map(|i| Some((i.bsd_name()?.to_string(), i.localized_display_name()?.to_string()))).collect()
}

pub(crate) fn index_of(dev: &str) -> u32 {
    let Ok(c) = std::ffi::CString::new(dev) else { return 0 };
    unsafe { libc::if_nametoindex(c.as_ptr()) }
}

pub(crate) fn name_of(index: u32) -> Option<String> {
    let mut buf = [0 as libc::c_char; libc::IF_NAMESIZE];
    let p = unsafe { libc::if_indextoname(index, buf.as_mut_ptr()) };
    (!p.is_null()).then(|| unsafe { std::ffi::CStr::from_ptr(p) }.to_string_lossy().into_owned())
}

/// What getifaddrs knows per device: the MAC in use (a private Wi-Fi address if one is on),
/// link speed, IPv6 addresses, and the IPv4 addresses (for picking the adapter a ping leaves by).
#[derive(Default, Debug)]
pub(crate) struct Link {
    pub mac: String,
    pub speed: u64,
    pub ipv6: Vec<String>,
    pub ipv4: Vec<Ipv4Addr>,
}

pub(crate) fn links() -> HashMap<String, Link> {
    let mut out: HashMap<String, Link> = HashMap::new();
    let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut head) } != 0 {
        return out;
    }
    let mut p = head;
    while !p.is_null() {
        let a = unsafe { &*p };
        p = a.ifa_next;
        if a.ifa_addr.is_null() {
            continue;
        }
        let name = unsafe { std::ffi::CStr::from_ptr(a.ifa_name) }.to_string_lossy().into_owned();
        let l = out.entry(name).or_default();
        match i32::from(unsafe { (*a.ifa_addr).sa_family }) {
            libc::AF_LINK => {
                let dl = unsafe { &*(a.ifa_addr as *const libc::sockaddr_dl) };
                let (n, len) = (dl.sdl_nlen as usize, dl.sdl_alen as usize);
                if len == 6 {
                    // sdl_data holds the name, then the address; it can run past the declared array.
                    let bytes = unsafe { std::slice::from_raw_parts((dl.sdl_data.as_ptr() as *const u8).add(n), len) };
                    l.mac = crate::net::mac_text(bytes);
                }
                if !a.ifa_data.is_null() {
                    l.speed = u64::from(unsafe { (*(a.ifa_data as *const libc::if_data)).ifi_baudrate });
                }
            }
            libc::AF_INET => {
                let sin = unsafe { &*(a.ifa_addr as *const libc::sockaddr_in) };
                l.ipv4.push(Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr)));
            }
            libc::AF_INET6 => {
                let sin6 = unsafe { &*(a.ifa_addr as *const libc::sockaddr_in6) };
                l.ipv6.push(std::net::Ipv6Addr::from(sin6.sin6_addr.s6_addr).to_string());
            }
            _ => {}
        }
    }
    unsafe { libc::freeifaddrs(head) };
    out
}

/// The joined Wi-Fi network on `dev`, through CoreWLAN. The network's name and the access point's
/// MAC come back only once Octet has Location permission (macOS counts them as location data).
#[derive(Default, Debug)]
pub(crate) struct Wlan {
    pub ssid: String,
    pub bssid: String,
    pub phy: String,
    pub mhz: u32,
    pub signal: u32,
    pub security: String,
    pub rate: u64,
}

pub(crate) fn wlan(dev: &str) -> Option<Wlan> {
    use objc2_core_wlan::{CWChannelBand, CWPHYMode, CWSecurity, CWWiFiClient};
    use objc2_foundation::NSString;
    unsafe {
        let i = CWWiFiClient::sharedWiFiClient().interfaceWithName(Some(&NSString::from_str(dev)))?;
        let rssi = i.rssiValue();
        let ch = i.wlanChannel();
        // Not joined: no channel and no signal.
        if rssi == 0 || ch.is_none() {
            return None;
        }
        let ch = ch?;
        let n = ch.channelNumber() as u32;
        let band = ch.channelBand();
        let mhz = match band {
            CWChannelBand::Band2GHz if n == 14 => 2484,
            CWChannelBand::Band2GHz => 2407 + 5 * n,
            CWChannelBand::Band5GHz => 5000 + 5 * n,
            CWChannelBand::Band6GHz => 5950 + 5 * n,
            _ => 0,
        };
        let phy = match i.activePHYMode() {
            CWPHYMode::Mode11a => "802.11a",
            CWPHYMode::Mode11b => "802.11b",
            CWPHYMode::Mode11g => "802.11g",
            CWPHYMode::Mode11n => "802.11n",
            CWPHYMode::Mode11ac => "802.11ac",
            CWPHYMode::Mode11ax => "802.11ax",
            m if m.0 == 7 => "802.11be",
            _ => "",
        };
        let security = match i.security() {
            CWSecurity::None => "Open",
            CWSecurity::WEP | CWSecurity::DynamicWEP => "WEP",
            CWSecurity::WPAPersonal | CWSecurity::WPAPersonalMixed => "WPA-Personal",
            CWSecurity::WPA2Personal | CWSecurity::Personal => "WPA2-Personal",
            CWSecurity::WPAEnterprise | CWSecurity::WPAEnterpriseMixed => "WPA-Enterprise",
            CWSecurity::WPA2Enterprise | CWSecurity::Enterprise => "WPA2-Enterprise",
            CWSecurity::WPA3Personal | CWSecurity(13) => "WPA3-Personal",   // 13: WPA3 transition
            CWSecurity::WPA3Enterprise => "WPA3-Enterprise",
            CWSecurity(14) | CWSecurity(15) => "Enhanced Open",
            _ => "",
        };
        Some(Wlan {
            ssid: i.ssid().map(|s| s.to_string()).unwrap_or_default(),
            bssid: i.bssid().map(|s| s.to_string().to_uppercase().replace(':', "-")).unwrap_or_default(),
            phy: phy.into(),
            mhz,
            // The same scale Windows uses: -100 dBm is 0 %, -50 dBm and better 100 %.
            signal: (2 * (rssi + 100)).clamp(0, 100) as u32,
            security: security.into(),
            rate: (i.transmitRate() * 1e6) as u64,
        })
    }
}

/// Asks once for Location permission, which is what lets Octet show the Wi-Fi network's name.
/// Only a bundled Octet.app can ask (the prompt's text is in its Info.plist); must run on the main thread.
pub fn ask_location() {
    use objc2_core_location::{CLAuthorizationStatus, CLLocationManager};
    // Run outside the app (cargo run), macOS would ask on behalf of the terminal instead.
    if !std::env::current_exe().is_ok_and(|e| e.to_string_lossy().contains(".app/Contents/MacOS/")) {
        return;
    }
    unsafe {
        let m = CLLocationManager::new();
        if m.authorizationStatus() == CLAuthorizationStatus::NotDetermined {
            m.requestWhenInUseAuthorization();
        }
        // The prompt belongs to the manager: it must outlive this call.
        std::mem::forget(m);
    }
}

// ---- small network helpers (net.rs on Windows) ----

pub fn elevated() -> bool {
    unsafe { libc::geteuid() == 0 }
}

pub fn computer_name() -> String {
    SCDynamicStore::local_host_name(None).map(|s| s.to_string()).unwrap_or_default()
}

/// The name a device on the network goes by (reverse DNS; mDNS answers for .local names). None when nobody answers.
pub fn hostname(ip: Ipv4Addr) -> Option<String> {
    let sa = sockaddr(ip, 0);
    let mut host = [0 as libc::c_char; 1025];
    let r = unsafe {
        libc::getnameinfo((&sa as *const libc::sockaddr_in).cast(), size_of::<libc::sockaddr_in>() as u32,
            host.as_mut_ptr(), host.len() as u32, std::ptr::null_mut(), 0, libc::NI_NAMEREQD)
    };
    let name = unsafe { std::ffi::CStr::from_ptr(host.as_ptr()) }.to_string_lossy().into_owned();
    (r == 0 && !name.is_empty()).then_some(name)
}

pub(crate) fn sockaddr(ip: Ipv4Addr, port: u16) -> libc::sockaddr_in {
    let mut sa: libc::sockaddr_in = unsafe { std::mem::zeroed() };
    sa.sin_len = size_of::<libc::sockaddr_in>() as u8;
    sa.sin_family = libc::AF_INET as u8;
    sa.sin_port = port.to_be();
    sa.sin_addr.s_addr = u32::from(ip).to_be();
    sa
}

/// Closes the socket when it goes out of scope.
struct Fd(i32);
impl Drop for Fd {
    fn drop(&mut self) {
        unsafe { libc::close(self.0) };
    }
}

/// Sends from `from` and out through the device that holds it. macOS routes by destination
/// alone, so the address isn't enough: IP_BOUND_IF pins the device.
fn bind_from(s: i32, from: Ipv4Addr) {
    if let Some(dev) = links().into_iter().find(|(_, l)| l.ipv4.contains(&from)).map(|(d, _)| d) {
        let idx = index_of(&dev);
        unsafe { libc::setsockopt(s, libc::IPPROTO_IP, libc::IP_BOUND_IF, (&idx as *const u32).cast(), 4) };
    }
    let sa = sockaddr(from, 0);
    unsafe { libc::bind(s, (&sa as *const libc::sockaddr_in).cast(), size_of::<libc::sockaddr_in>() as u32) };
}

/// Waits for `events` on `s`, at most `ms`. True when they came.
fn wait(s: i32, events: i16, ms: u32) -> bool {
    let mut p = libc::pollfd { fd: s, events, revents: 0 };
    unsafe { libc::poll(&mut p, 1, ms as i32) > 0 }
}

fn checksum(b: &[u8]) -> u16 {
    let mut sum: u32 = b.chunks(2).map(|c| u32::from(u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)]))).sum();
    while sum > 0xffff {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

/// One ICMP echo through an unprivileged ICMP socket; round-trip ms, or None after `timeout_ms`.
pub fn ping(ip: Ipv4Addr, from: Option<Ipv4Addr>, timeout_ms: u32) -> Option<u32> {
    use std::sync::atomic::{AtomicU16, Ordering};
    static SEQ: AtomicU16 = AtomicU16::new(1);
    let s = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, libc::IPPROTO_ICMP) };
    if s < 0 {
        return None;
    }
    let _fd = Fd(s);
    // A loopback target (a local DNS proxy) never answers a ping sent from the adapter's address.
    if let (Some(src), false) = (from, ip.is_loopback()) {
        bind_from(s, src);
    }
    // Every ping socket sees every echo reply: ours are told apart by id and sequence.
    let (id, seq) = (std::process::id() as u16, SEQ.fetch_add(1, Ordering::Relaxed));
    let mut pkt = [8u8, 0, 0, 0, 0, 0, 0, 0, b'o', b'c', b't', b'e', b't', 0];
    pkt[4..6].copy_from_slice(&id.to_be_bytes());
    pkt[6..8].copy_from_slice(&seq.to_be_bytes());
    let sum = checksum(&pkt);
    pkt[2..4].copy_from_slice(&sum.to_be_bytes());
    let sa = sockaddr(ip, 0);
    let t0 = Instant::now();
    if unsafe { libc::sendto(s, pkt.as_ptr().cast(), pkt.len(), 0, (&sa as *const libc::sockaddr_in).cast(), size_of::<libc::sockaddr_in>() as u32) } < 0 {
        return None;
    }
    let mut buf = [0u8; 1500];
    loop {
        let left = timeout_ms.saturating_sub(t0.elapsed().as_millis() as u32);
        if left == 0 || !wait(s, libc::POLLIN, left) {
            return None;
        }
        let mut from: libc::sockaddr_in = unsafe { std::mem::zeroed() };
        let mut len = size_of::<libc::sockaddr_in>() as u32;
        let n = unsafe { libc::recvfrom(s, buf.as_mut_ptr().cast(), buf.len(), 0, (&mut from as *mut libc::sockaddr_in).cast(), &mut len) };
        if n <= 0 {
            return None;
        }
        // The reply comes with its IP header in front.
        let b = &buf[..n as usize];
        let ihl = usize::from(b[0] & 0x0f) * 4;
        let Some(icmp) = b.get(ihl..ihl + 8) else { continue };
        if icmp[0] == 0 && icmp[4..6] == id.to_be_bytes() && icmp[6..8] == seq.to_be_bytes() && u32::from_be(from.sin_addr.s_addr) == u32::from(ip) {
            return Some(t0.elapsed().as_millis() as u32);
        }
    }
}

/// A TCP connect to `port`, sent like `ping`. Nothing is sent once connected.
pub fn tcping(ip: Ipv4Addr, port: u16, from: Option<Ipv4Addr>, timeout_ms: u32) -> Option<Tcp> {
    let s = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, libc::IPPROTO_TCP) };
    if s < 0 {
        return None;
    }
    let _fd = Fd(s);
    if let (Some(src), false) = (from, ip.is_loopback()) {
        bind_from(s, src);
    }
    unsafe { libc::fcntl(s, libc::F_SETFL, libc::fcntl(s, libc::F_GETFL) | libc::O_NONBLOCK) };
    let sa = sockaddr(ip, port);
    let t0 = Instant::now();
    let r = unsafe { libc::connect(s, (&sa as *const libc::sockaddr_in).cast(), size_of::<libc::sockaddr_in>() as u32) };
    if r != 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::EINPROGRESS) {
        return (std::io::Error::last_os_error().raw_os_error() == Some(libc::ECONNREFUSED)).then(|| Tcp::Closed(t0.elapsed().as_millis() as u32));
    }
    if r != 0 && !wait(s, libc::POLLOUT, timeout_ms) {
        return None;
    }
    let ms = t0.elapsed().as_millis() as u32;
    let (mut err, mut len) = (0i32, 4u32);
    unsafe { libc::getsockopt(s, libc::SOL_SOCKET, libc::SO_ERROR, (&mut err as *mut i32).cast(), &mut len) };
    match err {
        0 => Some(Tcp::Open(ms)),
        libc::ECONNREFUSED => Some(Tcp::Closed(ms)),
        _ => None,
    }
}

/// One resolved ARP entry. `heard_until`: when the device's last answer stops counting as recent
/// (seconds since 1970; macOS keeps it two minutes, `arp -l`'s Expire(I)), 0 when macOS keeps no such time.
#[derive(Debug)]
struct Arp {
    ip: Ipv4Addr,
    index: u32,
    mac: String,
    heard_until: u64,
}

impl Arp {
    /// Heard from lately (as Windows' Reachable), not merely remembered. No time kept: counts as heard.
    fn heard(&self) -> bool {
        self.heard_until == 0 || self.heard_until > UNIX_EPOCH.elapsed().map_or(0, |d| d.as_secs())
    }
}

/// <net/route.h>'s rt_msghdr_ext, what NET_RT_DUMPX_FLAGS gives: rt_msghdr's fields (wider), then rt_reach_info.
/// Laid out as the kernel's; only rcv_expire is read.
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct RtMsghdrExt {
    msglen: u16,
    version: u8,
    kind: u8,
    index: u32,
    flags: u32,
    reserved: u32,
    addrs: u32,
    pid: i32,
    seq: i32,
    errno: i32,
    used: u32,
    inits: u32,
    rmx: libc::rt_metrics,
    // rt_reach_info
    refcnt: u32,
    probes: u32,
    snd_expire: u64,
    rcv_expire: u64,
    rssi: i32,
    lqm: i32,
    npm: i32,
}
const NET_RT_DUMPX_FLAGS: i32 = 9;

/// The ARP table, each resolved entry with when it was last heard from. Falls back to the plain table
/// (nothing heard-from kept) if the extended one can't be read.
fn arp_table() -> Vec<Arp> {
    arp_dump(NET_RT_DUMPX_FLAGS, size_of::<RtMsghdrExt>())
        .or_else(|| arp_dump(libc::NET_RT_FLAGS, size_of::<libc::rt_msghdr>()))
        .unwrap_or_default()
}

/// One route dump of the ARP entries; `head`: the size of each message's header. None when it can't be read.
fn arp_dump(kind: i32, head: usize) -> Option<Vec<Arp>> {
    let mut mib = [libc::CTL_NET, libc::PF_ROUTE, 0, libc::AF_INET, kind, libc::RTF_LLINFO];
    let mut len = 0usize;
    if unsafe { libc::sysctl(mib.as_mut_ptr(), 6, std::ptr::null_mut(), &mut len, std::ptr::null_mut(), 0) } != 0 {
        return None;
    }
    let mut buf = vec![0u8; len + 4096];   // room for entries added in between
    len = buf.len();
    if unsafe { libc::sysctl(mib.as_mut_ptr(), 6, buf.as_mut_ptr().cast(), &mut len, std::ptr::null_mut(), 0) } != 0 {
        return None;
    }
    let mut out = vec![];
    let mut off = 0;
    // Each message: its header, then the address (sockaddr_in), then the link address (sockaddr_dl),
    // each sockaddr padded to 4 bytes. rtm_msglen leads both header kinds.
    while off + head <= len {
        let size = usize::from(u16::from_ne_bytes([buf[off], buf[off + 1]]));
        if size == 0 || off + size > len {
            break;
        }
        let heard_until = if head == size_of::<RtMsghdrExt>() {
            unsafe { std::ptr::read_unaligned(buf.as_ptr().add(off) as *const RtMsghdrExt) }.rcv_expire
        } else {
            0
        };
        let at = off + head;
        let sin = unsafe { std::ptr::read_unaligned(buf.as_ptr().add(at) as *const libc::sockaddr_in) };
        let pad = |n: usize| if n == 0 { 4 } else { (n + 3) & !3 };
        let dl_at = at + pad(usize::from(sin.sin_len));
        if dl_at + 8 <= off + size {
            let dl = &buf[dl_at..off + size];
            // sockaddr_dl: len, family, index (2), type, nlen, alen, slen, then name and address.
            let (index, nlen, alen) = (u16::from_ne_bytes([dl[2], dl[3]]), usize::from(dl[5]), usize::from(dl[6]));
            if alen == 6 && dl.len() >= 8 + nlen + 6 {
                let mac = &dl[8 + nlen..8 + nlen + 6];
                // Broadcast and multicast entries are no device.
                if mac != [0; 6] && mac[0] & 1 == 0 {
                    out.push(Arp { ip: Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr)), index: u32::from(index), mac: crate::net::mac_text(mac), heard_until });
                }
            }
        }
        off += size;
    }
    Some(out)
}

fn arp_entry(ip: Ipv4Addr, index: u32) -> Option<Arp> {
    arp_table().into_iter().find(|a| a.ip == ip && (index == 0 || a.index == index))
}

/// What the ARP table holds for `ip` on device `index`. Sends nothing.
pub fn neighbour(ip: Ipv4Addr, index: u32) -> Option<String> {
    arp_entry(ip, index).map(|a| a.mac)
}

/// Whether macOS has heard from `ip` in the last two minutes, not merely remembers it (entries live 20).
pub fn fresh(ip: Ipv4Addr, index: u32) -> bool {
    arp_entry(ip, index).is_some_and(|a| a.heard())
}

/// Asks again whether `ip` is there: a packet to a device macOS hasn't heard from lately makes it send a
/// unicast ARP probe first, and the answer marks the entry heard. The MAC once it is; None if nothing
/// answers within 4 s (macOS then drops the entry). Raw ARP would need root.
pub fn resolve(ip: Ipv4Addr, index: u32) -> Option<String> {
    if let Ok(u) = std::net::UdpSocket::bind("0.0.0.0:0") {
        use std::os::fd::AsRawFd;
        if index != 0 {
            unsafe { libc::setsockopt(u.as_raw_fd(), libc::IPPROTO_IP, libc::IP_BOUND_IF, (&index as *const u32).cast(), 4) };
        }
        let _ = u.send_to(b"octet", (ip, 9));   // the discard port
    }
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(4) {
        std::thread::sleep(Duration::from_millis(60));
        if let Some(a) = arp_entry(ip, index).filter(Arp::heard) {
            return Some(a.mac);
        }
    }
    None
}

/// Who answers ARP for `ip` (as SendARP does on Windows). Raw ARP needs root on macOS, so a UDP
/// packet to the discard port makes the system ask, and the answer is read from the ARP table.
pub fn arp(ip: Ipv4Addr) -> Option<String> {
    if let Some(m) = neighbour(ip, 0) {
        return Some(m);
    }
    if let Ok(u) = std::net::UdpSocket::bind("0.0.0.0:0") {
        let _ = u.send_to(b"octet", (ip, 9));
    }
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_millis(1200) {
        std::thread::sleep(Duration::from_millis(60));
        if let Some(m) = neighbour(ip, 0) {
            return Some(m);
        }
    }
    None
}

/// Calls `on_change` from its own thread when a link, an address, DNS, a lease or a service changes.
pub fn watch(on_change: fn()) {
    use objc2_core_foundation::{CFRunLoop, kCFRunLoopDefaultMode};
    use std::ffi::c_void;
    use std::ptr::NonNull;
    static CALLBACK: std::sync::OnceLock<fn()> = std::sync::OnceLock::new();
    let _ = CALLBACK.set(on_change);
    unsafe extern "C-unwind" fn changed(_: NonNull<SCDynamicStore>, _: NonNull<CFArray>, _: *mut c_void) {
        if let Some(f) = CALLBACK.get() { f() }
    }
    std::thread::spawn(|| unsafe {
        let Some(s) = SCDynamicStore::new(None, &CFString::from_static_str("Octet"), Some(changed), std::ptr::null_mut()) else { return };
        let pats: Vec<_> = ["State:/Network/Interface/[^/]+/(Link|IPv4)", "State:/Network/Service/[^/]+/(IPv4|DNS|DHCP)", "Setup:/Network/Service/.*", "State:/Network/Global/IPv4"]
            .iter().map(|p| CFString::from_str(p)).collect();
        let pats = CFArray::from_retained_objects(&pats);
        let pats: &CFArray = pats.as_ref();
        if !s.set_notification_keys(None, Some(pats)) {
            return;
        }
        let (Some(src), Some(lp)) = (SCDynamicStore::new_run_loop_source(None, &s, 0), CFRunLoop::current()) else { return };
        lp.add_source(Some(&src), kCFRunLoopDefaultMode);
        CFRunLoop::run();   // for the life of the process
    });
}

/// After networksetup sets an address: waits (up to 6 s) until it is on the device. macOS first
/// makes sure nobody else has it, so for a second or two there is no address and no route, and a
/// ping right after Apply would say the gateway isn't answering. Gives up quietly: the setting is saved.
pub fn wait_address(index: u32, address: Ipv4Addr) {
    let Some(dev) = name_of(index) else { return };
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(6) {
        if links().get(&dev).is_some_and(|l| l.ipv4.contains(&address)) {
            return;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

/// Asks for a fresh DHCP lease, as System Settings' Renew DHCP Lease does. Needs root.
/// macOS has no "release": the lease is given up only by turning DHCP off.
pub fn dhcp(index: u32, renew: bool) -> Result<(), String> {
    if !renew {
        return Err("On macOS Release turns the service's IPv4 off (networksetup), not this".into());
    }
    let dev = name_of(index).ok_or("That adapter is gone")?;
    // Released (IPv4 off): DHCP back on, then a moment for the lease, so the list shows the new address.
    if let Some((r, _)) = read()?.into_iter().find(|(r, _)| r.index == index && r.ipv4_off) {
        let o = crate::hidden("networksetup").args(["-setdhcp", &r.name]).output().map_err(|e| e.to_string())?;
        let said = String::from_utf8_lossy(&o.stdout);
        if !o.status.success() || said.contains("** Error") {
            return Err(format!("networksetup: {}", said.trim()));
        }
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(8) && links().get(&dev).is_none_or(|l| l.ipv4.is_empty()) {
            std::thread::sleep(Duration::from_millis(200));
        }
        return Ok(());
    }
    let all = SCNetworkInterface::all();
    let all: &CFArray<SCNetworkInterface> = unsafe { &*(&*all as *const CFArray).cast() };
    let i = all.iter().find(|i| i.bsd_name().is_some_and(|n| n.to_string() == dev)).ok_or("That adapter is gone")?;
    if i.force_configuration_refresh() { Ok(()) } else { Err("macOS did not renew the lease (needs administrator rights)".into()) }
}

// ---- Internet Sharing (System Settings > General > Sharing) ----

/// Internet Sharing's settings: what System Settings writes, and what configd's InternetSharingPreference
/// plugin acts on (it starts and stops the sharing daemon). Readable by anyone; writing needs root.
fn nat_prefs() -> Option<CFRetained<SCPreferences>> {
    SCPreferences::new(None, &CFString::from_static_str("Octet"), Some(&CFString::from_static_str("com.apple.nat.plist")))
}

/// macOS shares through a bridge of its own (bridge100 and up), not the port itself: the bridge holds this
/// Mac's address on the shared network, and the devices' ARP entries are on it.
fn share_bridge(dev: &str) -> bool {
    dev.strip_prefix("bridge").and_then(|n| n.parse::<u32>().ok()).is_some_and(|n| n >= 100)
}

/// What Internet Sharing is set to: on or off, from which device (`public`, an interface index), to which
/// (`private`), and the address this Mac has on the shared network once it runs (empty until then).
#[derive(serde::Serialize, Debug, Default, PartialEq)]
pub struct ShareState {
    pub on: bool,
    pub public: u32,
    pub private: Vec<u32>,
    pub address: String,
}

pub fn share_state() -> ShareState {
    let n = nat_prefs().and_then(|p| p.value(&CFString::from_static_str("NAT"))).map(|v| json(&v)).unwrap_or_default();
    let services = prefs(&["NetworkServices"]);
    let public_dev = n["PrimaryService"].as_str()
        .and_then(|id| services.get("NetworkServices")?[id]["Interface"]["DeviceName"].as_str().map(String::from))
        .or_else(|| n["PrimaryInterface"]["Device"].as_str().map(String::from))
        .unwrap_or_default();
    let address = links().into_iter().filter(|(d, _)| share_bridge(d)).find_map(|(_, l)| l.ipv4.first().copied());
    ShareState {
        on: n["Enabled"].as_i64() == Some(1) || n["Enabled"].as_bool() == Some(true),
        public: index_of(&public_dev),
        private: n["SharingDevices"].as_array().into_iter().flatten().filter_map(Value::as_str).map(index_of).filter(|&i| i != 0).collect(),
        address: address.map(|a| a.to_string()).unwrap_or_default(),
    }
}

/// Turns Internet Sharing on, from `public`'s service to `private`'s port with this Mac at `scope` (a.b.c.1;
/// the devices get a.b.c.2 and up), or off. As System Settings does it: the settings are written and applied,
/// and configd starts or stops sharing. The rest of what is there (System Settings' own keys) is kept.
/// Runs in the elevated copy.
pub fn share_set(public: u32, private: u32, on: bool, scope: Option<Ipv4Addr>) -> Result<(), String> {
    use objc2_core_foundation::CFMutableDictionary;
    let list = read()?;
    let primary = |i: u32| list.iter().find(|(r, _)| r.index == i);
    let (pubr, _) = primary(public).ok_or_else(|| format!("Sharing adapter {public} is gone"))?;
    let (privr, wifi) = primary(private).ok_or_else(|| format!("Sharing adapter {private} is gone"))?;
    if on && *wifi {
        return Err("Sharing out over Wi-Fi makes a hotspot; turn that on in System Settings > General > Sharing".into());
    }
    let p = nat_prefs().ok_or("Could not open the Internet Sharing settings")?;
    if !p.lock(true) {
        return Err("Could not change Internet Sharing (needs administrator rights)".into());
    }
    let key = CFString::from_static_str("NAT");
    let d = CFMutableDictionary::<CFString, CFType>::with_capacity(0);
    if let Some(old) = p.value(&key).and_then(|v| v.downcast::<CFDictionary>().ok()) {
        // SAFETY: a property-list dictionary; its keys are strings.
        let old: &CFDictionary<CFString, CFType> = unsafe { &*(&*old as *const CFDictionary).cast() };
        let (ks, vs) = old.to_vecs();
        for (k, v) in ks.iter().zip(&vs) {
            d.set(k, v);
        }
    }
    let s = |v: &str| CFString::from_str(v);
    let k = |v: &'static str| CFString::from_static_str(v);
    d.set(&k("Enabled"), &CFNumber::new_i32(i32::from(on)));
    if on {
        let dev = s(&pubr.description);
        let pi = CFDictionary::<CFString, CFType>::from_slices(
            &[&*k("Device"), &*k("Enabled"), &*k("HardwareKey"), &*k("PrimaryUserReadable")],
            &[&*dev, &*CFNumber::new_i32(0), &*s(""), &*s(&pubr.name)],
        );
        d.set(&k("PrimaryInterface"), &pi);
        d.set(&k("PrimaryService"), &s(&pubr.id));
        d.set(&k("SharingDevices"), &CFArray::from_retained_objects(&[s(&privr.description)]));
        match scope {
            Some(ip) => {
                let [a, b, c, _] = ip.octets();
                d.set(&k("SharingNetworkNumberStart"), &s(&format!("{a}.{b}.{c}.0")));
                d.set(&k("SharingNetworkNumberEnd"), &s(&format!("{a}.{b}.{c}.255")));
                d.set(&k("SharingNetworkMask"), &s("255.255.255.0"));
            }
            None => for x in ["SharingNetworkNumberStart", "SharingNetworkNumberEnd", "SharingNetworkMask"] {
                d.remove(&k(x));
            },
        }
    }
    // SAFETY: a dictionary of strings, numbers and arrays: a property list.
    let ok = unsafe { p.set_value(&key, &d) } && p.commit_changes() && p.apply_changes();
    p.unlock();
    if ok { Ok(()) } else { Err("macOS did not save the Internet Sharing settings".into()) }
}

/// The devices on the shared side, as Windows' neighbour table gives them: ARP entries on `index`'s port or
/// on Internet Sharing's bridge.
pub fn neighbours(index: u32) -> Vec<(Ipv4Addr, String)> {
    let bridges: Vec<u32> = links().into_keys().filter(|d| share_bridge(d)).map(|d| index_of(&d)).collect();
    arp_table().into_iter().filter(|a| a.index == index || bridges.contains(&a.index)).map(|a| (a.ip, a.mac)).collect()
}

/// GET https://{host}{path} with the system's curl: the body on a 200, else None.
pub fn https_get_with(host: &str, path: &str, mut progress: impl FnMut(f32)) -> Option<Vec<u8>> {
    use std::io::Read;
    let url = format!("https://{host}{path}");
    let curl = |extra: &[&str]| {
        let mut c = std::process::Command::new("/usr/bin/curl");
        c.args(["-sfL", "--proto", "=https", "--connect-timeout", "6", "-A", concat!("Octet/", env!("CARGO_PKG_VERSION"))]).args(extra).arg(&url);
        c
    };
    // The size first (after redirects), for the progress bar.
    let head = curl(&["-I"]).output().ok()?;
    let size: u64 = String::from_utf8_lossy(&head.stdout).lines().rev()
        .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").and_then(|v| v.trim().parse().ok()))
        .unwrap_or(0);
    let mut child = curl(&[]).stdout(std::process::Stdio::piped()).spawn().ok()?;
    let mut out = child.stdout.take()?;
    let (mut body, mut buf) = (Vec::new(), [0u8; 65536]);
    loop {
        match out.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                body.extend_from_slice(&buf[..n]);
                if size > 0 { progress(body.len() as f32 / size as f32); }
            }
            Err(_) => return None,
        }
    }
    child.wait().ok()?.success().then_some(body)
}

/// Runs `exe --elevated <request> -` as root through the system's administrator prompt and waits.
/// The elevated copy prints any failure (and exits 0) so the message comes back here intact.
pub fn sudo(exe: &std::path::Path, request_hex: &str) -> Result<(), String> {
    let quote = |s: &str| format!("'{}'", s.replace('\'', r"'\''"));
    // Only the exe path and hex digits go into the shell command.
    let sh = format!("{} --elevated {request_hex} -", quote(&exe.to_string_lossy()));
    let script = format!(
        "do shell script \"{}\" with prompt \"Octet wants to change your network settings.\" with administrator privileges",
        sh.replace('\\', r"\\").replace('"', "\\\"")
    );
    let o = std::process::Command::new("/usr/bin/osascript").args(["-e", &script]).output().map_err(|e| e.to_string())?;
    let err = String::from_utf8_lossy(&o.stderr);
    if !o.status.success() {
        // -128: the prompt was cancelled.
        return Err(if err.contains("-128") { "Cancelled: administrator permission was not given".into() } else { err.trim().to_string() });
    }
    // do shell script turns line breaks into carriage returns.
    match String::from_utf8_lossy(&o.stdout).replace('\r', "\n").trim() {
        "" => Ok(()),
        msg => Err(msg.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_services_in_settings_order() {
        let prefs = json!({
            "CurrentSet": "/Sets/S1",
            "Sets": { "S1": { "Network": { "Global": { "IPv4": { "ServiceOrder": ["W", "U", "H"] } },
                "Service": { "W": {}, "U": {}, "H": {}, "T": {}, "V": {}, "X": {} } } } },
            "NetworkServices": {
                "W": { "UserDefinedName": "Wi-Fi", "Interface": { "DeviceName": "en0", "Hardware": "AirPort", "Type": "IEEE80211" }, "IPv4": { "ConfigMethod": "DHCP" }, "DNS": {} },
                "X": { "UserDefinedName": "Dock LAN", "Interface": { "DeviceName": "en9", "Type": "Ethernet" }, "IPv4": { "ConfigMethod": "DHCP" },
                    "DNS": { "ServerAddresses": ["8.8.8.8", "8.8.4.4"] } },
                "U": { "UserDefinedName": "USB LAN", "Interface": { "DeviceName": "en7", "Type": "Ethernet" },
                    "IPv4": { "ConfigMethod": "Manual", "Addresses": ["192.168.50.20"], "SubnetMasks": ["255.255.255.0"], "Router": "192.168.50.254" },
                    "DNS": { "ServerAddresses": ["1.1.1.1", "fe80::1", "8.8.8.8"] } },
                "H": { "UserDefinedName": "Ethernet Adapter (en3)", "Interface": { "DeviceName": "en3", "HiddenConfiguration": true }, "IPv4": { "ConfigMethod": "DHCP" } },
                "T": { "UserDefinedName": "Thunderbolt Bridge", "__INACTIVE__": 1, "Interface": { "DeviceName": "bridge0" }, "IPv4": { "ConfigMethod": "DHCP" } },
                "V": { "UserDefinedName": "Work VPN", "Interface": { "Type": "VPN" }, "IPv4": { "ConfigMethod": "Manual" } }
            }
        });
        let live = json!({
            "State:/Network/Interface/en0/Link": { "Active": true },
            "State:/Network/Interface/en7/Link": { "Active": false },
            "State:/Network/Service/W/IPv4": { "Addresses": ["192.168.15.158"], "SubnetMasks": ["255.255.252.0"], "Router": "192.168.12.1" },
            "State:/Network/Service/W/DNS": { "ServerAddresses": ["171.102.211.106"], "DomainName": "lan" },
            "State:/Network/Service/W/DHCP": { "Option_54": [192, 168, 12, 1], "LeaseStartTime": 1000, "LeaseExpirationTime": 2000 },
            "State:/Network/Interface/en9/Link": { "Active": true },
            "State:/Network/Service/X/IPv4": { "Addresses": ["10.0.0.5"], "SubnetMasks": ["255.255.255.0"], "Router": "10.0.0.1" },
            "State:/Network/Service/X/DNS": { "ServerAddresses": ["10.0.0.1"] }
        });
        let (Value::Object(p), Value::Object(l)) = (prefs, live) else { unreachable!() };
        let s = services(&p, &l, &HashMap::new());
        let names: Vec<_> = s.iter().map(|r| r.0.name.as_str()).collect();
        assert_eq!(names, ["Wi-Fi", "USB LAN", "Thunderbolt Bridge", "Dock LAN"], "hidden and VPN services are left out; unordered ones come last");
        // Servers set by hand win over what DHCP still offers (it stays in the live DNS): those are what macOS uses.
        let m = &s.iter().find(|r| r.0.name == "Dock LAN").unwrap().0;
        assert_eq!((m.static_dns, m.dns.as_slice()), (true, &["8.8.8.8".to_string(), "8.8.4.4".into()][..]));
        let (w, wifi_card) = &s[0];
        assert!(w.dhcp && *wifi_card && !w.static_dns && w.status == "Up");
        assert_eq!((w.ip.as_slice(), w.gateway.as_slice(), w.dhcp_server.as_str()), (&["192.168.15.158".to_string()][..], &["192.168.12.1".to_string()][..], "192.168.12.1"));
        assert_eq!((w.lease_got, w.lease_ends, w.suffix.as_str()), (1000, 2000, "lan"));
        let u = &s[1].0;
        assert_eq!((u.status.as_str(), u.dhcp, u.static_dns), ("Disconnected", false, true));
        assert_eq!(u.ip, ["192.168.50.20"], "a static service that is down shows what it is set to");
        assert_eq!(u.dns, ["1.1.1.1", "8.8.8.8"]);
        assert_eq!(s[2].0.status, "Disabled");
    }

    /// An extra-address service Octet made reads as one more address (and gateway) of the first service on its
    /// port; a look-alike that doesn't match its own address stays an adapter. IPv4 Off (Release) is kept, empty.
    #[test]
    fn extra_services_fold_into_the_first() {
        let svc = |name: &str, dev: &str, v4: Value| json!({ "UserDefinedName": name, "Interface": { "DeviceName": dev, "Type": "Ethernet" }, "IPv4": v4 });
        let manual = |ip: &str, router: Option<&str>| {
            let mut v = json!({ "ConfigMethod": "Manual", "Addresses": [ip], "SubnetMasks": ["255.255.255.0"] });
            if let Some(r) = router { v["Router"] = json!(r); }
            v
        };
        let prefs = json!({
            "CurrentSet": "/Sets/S1",
            "Sets": { "S1": { "Network": { "Global": { "IPv4": { "ServiceOrder": ["A", "X", "Y", "O"] } }, "Service": {} } } },
            "NetworkServices": {
                "A": svc("USB LAN", "en7", manual("192.168.1.5", Some("192.168.1.1"))),
                "X": svc("USB LAN + 10.0.0.2", "en7", manual("10.0.0.2", Some("10.0.0.1"))),
                "Y": svc("USB LAN + 10.9.9.9", "en7", manual("10.0.0.3", None)),
                "O": svc("Dock", "en8", json!({ "ConfigMethod": "DHCP", "__INACTIVE__": true }))
            }
        });
        let (Value::Object(p), Value::Object(l)) = (prefs, json!({})) else { unreachable!() };
        let s = services(&p, &l, &HashMap::new());
        let names: Vec<_> = s.iter().map(|r| r.0.name.as_str()).collect();
        assert_eq!(names, ["USB LAN", "USB LAN + 10.9.9.9", "Dock"]);
        // macOS's own name for a service wins over the stored one (networksetup knows it by that).
        let renamed = services(&p, &l, &HashMap::from([("O".to_string(), "Dock Ethernet".to_string())]));
        assert_eq!(renamed[2].0.name, "Dock Ethernet");
        let a = &s[0].0;
        assert_eq!((a.ip.as_slice(), a.gateway.as_slice()), (&["192.168.1.5".to_string(), "10.0.0.2".into()][..], &["192.168.1.1".to_string(), "10.0.0.1".into()][..]));
        let o = &s[2].0;
        assert!(o.ipv4_off && o.dhcp && o.ip.is_empty(), "released: DHCP with no address");
    }

    /// The kernel's rt_msghdr_ext: 40 bytes of header, 56 of rt_metrics, then rt_reach_info (u64s 8-aligned).
    #[test]
    fn route_message_layout() {
        assert_eq!(std::mem::offset_of!(RtMsghdrExt, rmx), 40);
        assert_eq!(std::mem::offset_of!(RtMsghdrExt, rcv_expire), 112);
        assert_eq!(size_of::<RtMsghdrExt>(), 136);
    }

    /// The heard-from times read here are the ones `arp -anl` shows as Expire(I):
    /// `cargo test -p octet arp_heard_matches_arp_l -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn arp_heard_matches_arp_l() {
        let now = UNIX_EPOCH.elapsed().unwrap().as_secs();
        let out = String::from_utf8(std::process::Command::new("arp").arg("-anl").output().unwrap().stdout).unwrap();
        let mut checked = 0;
        for a in arp_table() {
            let line = out.lines().find(|l| l.split_whitespace().next() == Some(&a.ip.to_string())).unwrap();
            let col = line.split_whitespace().nth(3).unwrap();   // Expire(I)
            let left = a.heard_until.saturating_sub(now);
            println!("{:<16} {} heard_until {} ({left}s left)  arp: {col}", a.ip, a.mac, a.heard_until);
            match col {
                "expired" => assert!(a.heard_until != 0 && !a.heard(), "{line}"),
                "(none)" => assert_eq!(a.heard_until, 0, "{line}"),
                _ => assert!(a.heard() && left > 0 && left <= 130, "{line}"),
            }
            checked += 1;
        }
        assert!(checked > 0, "no ARP entries to check");
    }

    /// Reads this Mac: `cargo test -p octet reads_this_mac -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn reads_this_mac() {
        for r in read().unwrap() {
            println!("{r:?}");
        }
        println!("{:?}", links().get("en0"));
        println!("{:?}", wlan("en0"));
        println!("{:?}", display_names());
        println!("ARP {:?}", arp_table());
        println!("SHARING {:?}  shared side {:?}", share_state(), neighbours(0));
        // Out through the adapter with the default route: its gateway answers ping, ARP and (usually) port 80.
        if let Some(a) = crate::adapters::read().unwrap().into_iter().find(|a| !a.cfg.gateway.is_empty()) {
            let (gw, me): (Ipv4Addr, Ipv4Addr) = (a.cfg.gateway.parse().unwrap(), a.cfg.address.parse().unwrap());
            let t = Instant::now();
            println!("PING {gw} from {me}: {:?}  ARP: {:?} ({} ms)", ping(gw, Some(me), 1500), arp(gw), t.elapsed().as_millis());
            println!("TCP {gw}:80 {:?}  :1 {:?}", tcping(gw, 80, Some(me), 1500), tcping(gw, 1, Some(me), 800));
            println!("SCAN gateway {:?}  nobody {:?}", crate::net::scan_host(gw, Some(me), a.index), crate::net::scan_host(Ipv4Addr::from(u32::from(me) ^ 0x3f), Some(me), a.index));
            println!("NAME {:?}  this Mac {:?}", hostname(gw), computer_name());
        }
    }
}
