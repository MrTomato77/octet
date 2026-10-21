//! The macOS side: the same answers net.rs and adapters.rs give on Windows. An "adapter" is a
//! network service (what System Settings lists: "Wi-Fi", "USB 10/100/1000 LAN"), read from
//! SystemConfiguration's preferences (what is set) and dynamic store (what is live).
//! Ping, TCP and the ARP table use BSD sockets directly; none of the reading needs root.
//! Changes run networksetup in a second copy of Octet, started as root through the system's
//! administrator prompt (see `sudo`).

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

/// The network services of the current location, in System Settings' order, as Windows-shaped rows.
/// Services macOS hides (HiddenConfiguration), ones without a network device (VPNs, PPP) and
/// ones with IPv4 turned off are left out: there is nothing in them for Octet to edit.
pub(crate) fn services(prefs: &Map<String, Value>, live: &Map<String, Value>) -> Vec<Raw> {
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
        if s.is_null() || iface["HiddenConfiguration"].as_bool() == Some(true) || matches!(iface["Type"].as_str(), Some("PPP" | "VPN" | "IPSec"))
            || !matches!(method, "DHCP" | "BOOTP" | "Manual" | "INFORM") {
            continue;
        }
        let inactive = s["__INACTIVE__"].as_i64().is_some_and(|n| n != 0) || s["__INACTIVE__"].as_bool() == Some(true);
        let ipv4 = state(format!("State:/Network/Service/{id}/IPv4"));
        let dns = state(format!("State:/Network/Service/{id}/DNS"));
        let dhcp = state(format!("State:/Network/Service/{id}/DHCP"));
        // No Link key (a bridge with nothing in it): connected when it has an address.
        let up = state(format!("State:/Network/Interface/{dev}/Link"))["Active"].as_bool().unwrap_or(!ipv4.is_null());
        let auto = matches!(method, "DHCP" | "BOOTP");
        let static_dns = !ips(&s["DNS"]["ServerAddresses"]).is_empty();
        let mut raw = Raw {
            id: id.clone(),
            index: index_of(dev),
            name: s["UserDefinedName"].as_str().or(iface["UserDefinedName"].as_str()).unwrap_or(dev).to_string(),
            description: dev.to_string(),
            status: if inactive { "Disabled" } else if up { "Up" } else { "Disconnected" }.into(),
            dhcp: auto,
            static_dns,
            wifi_card: iface["Type"].as_str() == Some("IEEE80211") || iface["Hardware"].as_str() == Some("AirPort"),
            ..Default::default()
        };
        if !inactive {
            // Live values when the service is up; a static service that isn't keeps what it is set to.
            let src = if ipv4.is_null() && !auto { v4 } else { ipv4 };
            (raw.ip, raw.subnet) = (ips(&src["Addresses"]), ips(&src["SubnetMasks"]));
            raw.gateway = src["Router"].as_str().filter(|r| r.parse::<Ipv4Addr>().is_ok()).map(|r| vec![r.to_string()]).unwrap_or_default();
            raw.dns = if dns.is_null() { ips(&s["DNS"]["ServerAddresses"]) } else { ips(&dns["ServerAddresses"]) };
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
        out.push(raw);
    }
    out
}

/// What the adapter reader needs that the service list doesn't say.
#[derive(Default, Debug)]
pub(crate) struct Raw {
    pub id: String,
    pub index: u32,
    pub name: String,
    pub description: String,
    pub status: String,
    pub dhcp: bool,
    pub static_dns: bool,
    pub dhcp_server: String,
    pub ip: Vec<String>,
    pub subnet: Vec<String>,
    pub gateway: Vec<String>,
    pub dns: Vec<String>,
    pub suffix: String,
    pub lease_got: u64,
    pub lease_ends: u64,
    pub wifi_card: bool,
}

pub(crate) fn read() -> Result<Vec<Raw>, String> {
    let p = prefs(&["CurrentSet", "Sets", "NetworkServices"]);
    if p.is_empty() {
        return Err("Could not read the network settings".into());
    }
    let l = live(&["State:/Network/Service/[^/]+/(IPv4|DNS|DHCP)", "State:/Network/Interface/[^/]+/Link"]);
    Ok(services(&p, &l))
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
                    l.mac = bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join("-");
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

fn sockaddr(ip: Ipv4Addr, port: u16) -> libc::sockaddr_in {
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

/// The ARP table: (address, device index, MAC as "AA-BB-..") for every resolved entry.
fn arp_table() -> Vec<(Ipv4Addr, u32, String)> {
    let mut mib = [libc::CTL_NET, libc::PF_ROUTE, 0, libc::AF_INET, libc::NET_RT_FLAGS, libc::RTF_LLINFO];
    let mut len = 0usize;
    if unsafe { libc::sysctl(mib.as_mut_ptr(), 6, std::ptr::null_mut(), &mut len, std::ptr::null_mut(), 0) } != 0 {
        return vec![];
    }
    let mut buf = vec![0u8; len + 4096];   // room for entries added in between
    len = buf.len();
    if unsafe { libc::sysctl(mib.as_mut_ptr(), 6, buf.as_mut_ptr().cast(), &mut len, std::ptr::null_mut(), 0) } != 0 {
        return vec![];
    }
    let mut out = vec![];
    let mut off = 0;
    // Each message: rt_msghdr, then the address (sockaddr_in), then the link address (sockaddr_dl),
    // each sockaddr padded to 4 bytes.
    while off + size_of::<libc::rt_msghdr>() <= len {
        let rtm = unsafe { std::ptr::read_unaligned(buf.as_ptr().add(off) as *const libc::rt_msghdr) };
        let size = usize::from(rtm.rtm_msglen);
        if size == 0 || off + size > len {
            break;
        }
        let at = off + size_of::<libc::rt_msghdr>();
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
                    out.push((Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr)), u32::from(index), mac.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join("-")));
                }
            }
        }
        off += size;
    }
    out
}

/// What the ARP table holds for `ip` on device `index`. Sends nothing.
pub fn neighbour(ip: Ipv4Addr, index: u32) -> Option<String> {
    arp_table().into_iter().find(|(a, i, _)| *a == ip && (index == 0 || *i == index)).map(|(_, _, m)| m)
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

/// Asks for a fresh DHCP lease, as System Settings' Renew DHCP Lease does. Needs root.
/// macOS has no "release": the lease is given up only by turning DHCP off.
pub fn dhcp(index: u32, renew: bool) -> Result<(), String> {
    if !renew {
        return Err("macOS can't release a lease on its own; use Renew".into());
    }
    let dev = name_of(index).ok_or("That adapter is gone")?;
    let all = SCNetworkInterface::all();
    let all: &CFArray<SCNetworkInterface> = unsafe { &*(&*all as *const CFArray).cast() };
    let i = all.iter().find(|i| i.bsd_name().is_some_and(|n| n.to_string() == dev)).ok_or("That adapter is gone")?;
    if i.force_configuration_refresh() { Ok(()) } else { Err("macOS did not renew the lease (needs administrator rights)".into()) }
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
                "Service": { "W": {}, "U": {}, "H": {}, "T": {}, "V": {} } } } },
            "NetworkServices": {
                "W": { "UserDefinedName": "Wi-Fi", "Interface": { "DeviceName": "en0", "Hardware": "AirPort", "Type": "IEEE80211" }, "IPv4": { "ConfigMethod": "DHCP" }, "DNS": {} },
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
            "State:/Network/Service/W/DHCP": { "Option_54": [192, 168, 12, 1], "LeaseStartTime": 1000, "LeaseExpirationTime": 2000 }
        });
        let (Value::Object(p), Value::Object(l)) = (prefs, live) else { unreachable!() };
        let s = services(&p, &l);
        let names: Vec<_> = s.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["Wi-Fi", "USB LAN", "Thunderbolt Bridge"], "hidden and VPN services are left out; unordered ones come last");
        let w = &s[0];
        assert!(w.dhcp && w.wifi_card && !w.static_dns && w.status == "Up");
        assert_eq!((w.ip.as_slice(), w.gateway.as_slice(), w.dhcp_server.as_str()), (&["192.168.15.158".to_string()][..], &["192.168.12.1".to_string()][..], "192.168.12.1"));
        assert_eq!((w.lease_got, w.lease_ends, w.suffix.as_str()), (1000, 2000, "lan"));
        let u = &s[1];
        assert_eq!((u.status.as_str(), u.dhcp, u.static_dns), ("Disconnected", false, true));
        assert_eq!(u.ip, ["192.168.50.20"], "a static service that is down shows what it is set to");
        assert_eq!(u.dns, ["1.1.1.1", "8.8.8.8"]);
        assert_eq!(s[2].status, "Disabled");
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
