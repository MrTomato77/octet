//! Small Windows network helpers: elevation check, ARP probe, ping, change notifications.

use std::net::Ipv4Addr;

/// IPv4 in the byte order the IP Helper API wants (network order in a native u32).
fn ip_u32(ip: Ipv4Addr) -> u32 {
    u32::from_ne_bytes(ip.octets())
}

#[cfg(windows)]
pub fn elevated() -> bool {
    unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
}
#[cfg(not(windows))]
pub fn elevated() -> bool {
    false
}

/// Who answers ARP for `ip` on an attached subnet, as "AA-BB-..". None = nobody (or not reachable by ARP).
#[cfg(windows)]
pub fn arp(ip: Ipv4Addr) -> Option<String> {
    let mut mac = [0u8; 8];
    let mut len = mac.len() as u32;
    let r = unsafe { windows_sys::Win32::NetworkManagement::IpHelper::SendARP(ip_u32(ip), 0, mac.as_mut_ptr().cast(), &mut len) };
    (r == 0 && len >= 6).then(|| mac[..len as usize].iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join("-"))
}
#[cfg(not(windows))]
pub fn arp(_: Ipv4Addr) -> Option<String> {
    None
}

/// The name a device on the network goes by (reverse DNS, which on Windows
/// also asks LLMNR and NetBIOS). None when nobody answers.
#[cfg(windows)]
pub fn hostname(ip: Ipv4Addr) -> Option<String> {
    use windows_sys::Win32::Networking::WinSock::{AF_INET, GetNameInfoW, NI_NAMEREQD, SOCKADDR, SOCKADDR_IN, WSADATA, WSAStartup};
    unsafe {
        let mut wsa: WSADATA = std::mem::zeroed();
        WSAStartup(0x0202, &mut wsa);   // reference-counted; once per call is fine
        let mut sa: SOCKADDR_IN = std::mem::zeroed();
        sa.sin_family = AF_INET;
        sa.sin_addr.S_un.S_addr = ip_u32(ip);
        let mut host = [0u16; 1025];
        let r = GetNameInfoW((&sa as *const SOCKADDR_IN).cast::<SOCKADDR>(), size_of::<SOCKADDR_IN>() as i32,
            host.as_mut_ptr(), host.len() as u32, std::ptr::null_mut(), 0, NI_NAMEREQD as i32);
        let len = host.iter().position(|&c| c == 0)?;
        (r == 0 && len > 0).then(|| String::from_utf16_lossy(&host[..len]))
    }
}
#[cfg(not(windows))]
pub fn hostname(_: Ipv4Addr) -> Option<String> {
    None
}

/// This computer's name, as others on the network see it ("MRTOMATO-PC").
#[cfg(windows)]
pub fn computer_name() -> String {
    use windows_sys::Win32::System::SystemInformation::{ComputerNamePhysicalNetBIOS, GetComputerNameExW};
    let mut buf = [0u16; 256];
    let mut len = buf.len() as u32;
    if unsafe { GetComputerNameExW(ComputerNamePhysicalNetBIOS, buf.as_mut_ptr(), &mut len) } == 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..len as usize])
}
#[cfg(not(windows))]
pub fn computer_name() -> String {
    String::new()
}

/// The address the internet sees, with who it belongs to. Asked only when the user clicks
/// (it leaves this machine): one request to ipinfo.io through curl.exe, which Windows ships.
#[derive(serde::Serialize, Debug, PartialEq)]
pub struct Public {
    pub ip: String,
    pub isp: String,
    pub city: String,
}
pub fn public_ip() -> Result<Public, String> {
    let o = crate::hidden("curl.exe").args(["-s", "--max-time", "6", "https://ipinfo.io/json"]).output().map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err("Could not reach the internet".into());
    }
    parse_public(&o.stdout)
}
/// ipinfo's answer: {"ip": "...", "city": "...", "org": "AS7470 True Internet ..."}.
fn parse_public(json: &[u8]) -> Result<Public, String> {
    let v: serde_json::Value = serde_json::from_slice(json).map_err(|_| "Unexpected answer".to_string())?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let ip = s("ip");
    if ip.parse::<std::net::IpAddr>().is_err() {
        return Err("Unexpected answer".into());
    }
    // "AS7470 True Internet Co.,Ltd." -> "True Internet Co.,Ltd."
    let org = s("org");
    let isp = org.split_once(' ').filter(|(a, _)| a.starts_with("AS")).map_or(org.as_str(), |(_, b)| b).to_string();
    Ok(Public { ip, isp, city: s("city") })
}

/// Who made the network card, from the first 3 bytes of its MAC ("00-11-32-.." -> "Synology").
/// The IEEE list is built into the exe (data/oui.txt), so nothing is looked up online.
pub fn vendor(mac: &str) -> Option<&'static str> {
    static OUI: &str = include_str!("../data/oui.txt");
    let key: String = mac.chars().filter(char::is_ascii_hexdigit).take(6).collect::<String>().to_ascii_uppercase();
    if key.len() < 6 {
        return None;
    }
    // ponytail: linear scan of ~40k lines (about a millisecond); a sorted index if it ever runs in a loop.
    OUI.lines().find_map(|l| l.strip_prefix(key.as_str())?.strip_prefix('\t'))
}

/// Empties the DNS client cache, as `ipconfig /flushdns` does (it calls this same function),
/// without starting a process: under 1 ms instead of ~45 ms.
#[cfg(windows)]
pub fn flush_dns() -> bool {
    #[link(name = "dnsapi")]
    unsafe extern "system" {
        fn DnsFlushResolverCache() -> i32;
    }
    unsafe { DnsFlushResolverCache() != 0 }
}
#[cfg(not(windows))]
pub fn flush_dns() -> bool {
    false
}

/// One ICMP echo; round-trip milliseconds, or None when there is no reply in `timeout_ms`.
/// With `from`, it is sent from that address, so it leaves through the adapter that holds it
/// rather than whichever one has the default route.
#[cfg(windows)]
pub fn ping(ip: Ipv4Addr, from: Option<Ipv4Addr>, timeout_ms: u32) -> Option<u32> {
    use windows_sys::Win32::NetworkManagement::IpHelper::{IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho2Ex};
    unsafe {
        let h = IcmpCreateFile();
        if h.is_null() || h as isize == -1 {
            return None;
        }
        let data = *b"octet";
        // ICMP_ECHO_REPLY (40 bytes on x64) + payload + 8 bytes for an ICMP error, per the docs.
        let mut reply = [0u8; 128];
        let src = from.map_or(0, ip_u32);
        let n = IcmpSendEcho2Ex(h, std::ptr::null_mut(), None, std::ptr::null(), src, ip_u32(ip), data.as_ptr().cast(), data.len() as u16,
            std::ptr::null(), reply.as_mut_ptr().cast(), reply.len() as u32, timeout_ms);
        IcmpCloseHandle(h);
        // ICMP_ECHO_REPLY: Address u32, Status u32, RoundTripTime u32, ...
        let status = u32::from_ne_bytes(reply[4..8].try_into().ok()?);
        let rtt = u32::from_ne_bytes(reply[8..12].try_into().ok()?);
        (n > 0 && status == 0).then_some(rtt)
    }
}
#[cfg(not(windows))]
pub fn ping(_: Ipv4Addr, _: Option<Ipv4Addr>, _: u32) -> Option<u32> {
    None
}

/// What the neighbour (ARP) cache holds for `ip` on interface `index`, as "AA-BB-..".
/// Sends nothing: it reads what Windows already learned (say, while a ping went out).
#[cfg(windows)]
#[allow(non_upper_case_globals)]   // Windows names these Nlns*
pub fn neighbour(ip: Ipv4Addr, index: u32) -> Option<String> {
    use windows_sys::Win32::NetworkManagement::IpHelper::{GetIpNetEntry2, MIB_IPNET_ROW2};
    use windows_sys::Win32::Networking::WinSock::{AF_INET, NlnsIncomplete, NlnsUnreachable};
    let mut row = MIB_IPNET_ROW2::default();
    row.Address.Ipv4.sin_family = AF_INET;
    row.Address.Ipv4.sin_addr.S_un.S_addr = ip_u32(ip);
    row.InterfaceIndex = index;
    if unsafe { GetIpNetEntry2(&mut row) } != 0 || matches!(row.State, NlnsIncomplete | NlnsUnreachable) || row.PhysicalAddressLength != 6 {
        return None;
    }
    let mac = &row.PhysicalAddress[..6];
    (mac != [0; 6]).then(|| mac.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join("-"))
}
#[cfg(not(windows))]
pub fn neighbour(_: Ipv4Addr, _: u32) -> Option<String> {
    None
}

/// Who holds an address, for the Scan grid.
#[derive(serde::Serialize, Debug, PartialEq)]
pub struct Host {
    pub mac: Option<String>,
    pub vendor: Option<&'static str>,
    pub ms: Option<u32>,
}

/// One short ping from the adapter, then the neighbour cache, which also catches devices
/// that ignore ping (most Windows PCs) since the ping made Windows ask ARP for them.
/// ponytail: ~400 ms for an empty address; the UI runs many at once.
pub fn scan_host(ip: Ipv4Addr, from: Option<Ipv4Addr>, index: u32) -> Option<Host> {
    let ms = ping(ip, from, 400);
    let mac = neighbour(ip, index);
    (ms.is_some() || mac.is_some()).then(|| Host { vendor: mac.as_deref().and_then(vendor), mac, ms })
}

/// Calls `on_change` from a Windows thread whenever an interface or an IPv4
/// address changes (cable in/out, Wi-Fi joins, DHCP lease, someone else's netsh).
#[cfg(windows)]
pub fn watch(on_change: fn()) {
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        MIB_IPINTERFACE_ROW, MIB_NOTIFICATION_TYPE, MIB_UNICASTIPADDRESS_ROW, NotifyIpInterfaceChange, NotifyUnicastIpAddressChange,
    };
    static CALLBACK: std::sync::OnceLock<fn()> = std::sync::OnceLock::new();
    let _ = CALLBACK.set(on_change);
    unsafe extern "system" fn iface(_: *const core::ffi::c_void, _: *const MIB_IPINTERFACE_ROW, _: MIB_NOTIFICATION_TYPE) {
        if let Some(f) = CALLBACK.get() { f() }
    }
    unsafe extern "system" fn addr(_: *const core::ffi::c_void, _: *const MIB_UNICASTIPADDRESS_ROW, _: MIB_NOTIFICATION_TYPE) {
        if let Some(f) = CALLBACK.get() { f() }
    }
    const AF_INET: u16 = 2;
    // Registered for the life of the process; the handles are never cancelled.
    let (mut h1, mut h2): (HANDLE, HANDLE) = (std::ptr::null_mut(), std::ptr::null_mut());
    unsafe {
        NotifyIpInterfaceChange(AF_INET, Some(iface), std::ptr::null(), false, &mut h1);
        NotifyUnicastIpAddressChange(AF_INET, Some(addr), std::ptr::null(), false, &mut h2);
    }
}
#[cfg(not(windows))]
pub fn watch(_: fn()) {}

#[cfg(test)]
mod tests {
    #[test]
    fn loopback_answers_ping() {
        assert!(super::ping(std::net::Ipv4Addr::LOCALHOST, None, 1000).is_some());
        assert!(super::ping(std::net::Ipv4Addr::LOCALHOST, Some(std::net::Ipv4Addr::LOCALHOST), 1000).is_some());
    }

    #[test]
    fn scan_finds_loopback_and_skips_nobody() {
        let lo = super::scan_host(std::net::Ipv4Addr::LOCALHOST, None, 1).expect("loopback answers");
        assert!(lo.ms.is_some());
        // TEST-NET-1 is never assigned, so nothing answers and nothing is cached.
        assert_eq!(super::scan_host(std::net::Ipv4Addr::new(192, 0, 2, 77), None, 1), None);
    }

    #[test]
    fn reads_ipinfo_answers() {
        let p = super::parse_public(br#"{"ip":"203.0.113.48","city":"Bangkok","org":"AS7470 True Internet Co.,Ltd."}"#).unwrap();
        assert_eq!((p.ip.as_str(), p.isp.as_str(), p.city.as_str()), ("203.0.113.48", "True Internet Co.,Ltd.", "Bangkok"));
        assert!(super::parse_public(b"<html>rate limited</html>").is_err());
        assert!(super::parse_public(br#"{"ip":"not an ip"}"#).is_err());
        assert!(!super::computer_name().is_empty());
    }

    #[test]
    fn flushes_dns_cache() {
        let t = std::time::Instant::now();
        assert!(super::flush_dns());
        println!("TIME flush_dns ms: {:.2}", t.elapsed().as_secs_f64() * 1000.0);
    }

    #[test]
    fn names_card_makers() {
        assert_eq!(super::vendor("00-11-32-AB-CD-EF"), Some("Synology"));
        assert_eq!(super::vendor("b8:27:eb:00:00:01"), Some("Raspberry Pi Foundation"));
        assert_eq!(super::vendor("02-00-00-00-00-01"), None);
        assert_eq!(super::vendor(""), None);
    }

    #[test]
    fn loopback_has_a_name() {
        assert!(super::hostname(std::net::Ipv4Addr::LOCALHOST).is_some());
    }
}
