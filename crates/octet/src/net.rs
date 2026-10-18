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
#[cfg(windows)]
pub fn ping(ip: Ipv4Addr, timeout_ms: u32) -> Option<u32> {
    use windows_sys::Win32::NetworkManagement::IpHelper::{IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho};
    unsafe {
        let h = IcmpCreateFile();
        if h.is_null() || h as isize == -1 {
            return None;
        }
        let data = *b"octet";
        // ICMP_ECHO_REPLY (40 bytes on x64) + payload + 8 bytes for an ICMP error, per the docs.
        let mut reply = [0u8; 128];
        let n = IcmpSendEcho(h, ip_u32(ip), data.as_ptr().cast(), data.len() as u16, std::ptr::null(),
            reply.as_mut_ptr().cast(), reply.len() as u32, timeout_ms);
        IcmpCloseHandle(h);
        // ICMP_ECHO_REPLY: Address u32, Status u32, RoundTripTime u32, ...
        let status = u32::from_ne_bytes(reply[4..8].try_into().ok()?);
        let rtt = u32::from_ne_bytes(reply[8..12].try_into().ok()?);
        (n > 0 && status == 0).then_some(rtt)
    }
}
#[cfg(not(windows))]
pub fn ping(_: Ipv4Addr, _: u32) -> Option<u32> {
    None
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
        assert!(super::ping(std::net::Ipv4Addr::LOCALHOST, 1000).is_some());
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
