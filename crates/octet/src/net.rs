//! Small network helpers: elevation check, ARP probe, ping, change notifications.
//! Windows here; the macOS versions are in mac.rs.

use std::net::Ipv4Addr;

#[cfg(target_os = "macos")]
pub use crate::mac::{arp, computer_name, dhcp, elevated, hostname, https_get_with, neighbour, ping, tcping, watch};

/// IPv4 in the byte order the IP Helper API wants (network order in a native u32).
#[cfg(windows)]
fn ip_u32(ip: Ipv4Addr) -> u32 {
    u32::from_ne_bytes(ip.octets())
}

#[cfg(windows)]
pub fn elevated() -> bool {
    unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
}

/// Who answers ARP for `ip` on an attached subnet, as "AA-BB-..". None = nobody (or not reachable by ARP).
#[cfg(windows)]
pub fn arp(ip: Ipv4Addr) -> Option<String> {
    let mut mac = [0u8; 8];
    let mut len = mac.len() as u32;
    let r = unsafe { windows_sys::Win32::NetworkManagement::IpHelper::SendARP(ip_u32(ip), 0, mac.as_mut_ptr().cast(), &mut len) };
    (r == 0 && len >= 6).then(|| mac[..len as usize].iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join("-"))
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

/// GET https://{host}{path}: the body when the answer is 200, else None. Windows' own HTTP
/// stack, so the system proxy and certificate store apply; gives up after 6 s per step.
pub fn https_get(host: &str, path: &str) -> Option<Vec<u8>> {
    https_get_with(host, path, |_| {})
}

/// Like `https_get`, telling `progress` how much has come (0 to 1) when the size is known.
#[cfg(windows)]
pub fn https_get_with(host: &str, path: &str, mut progress: impl FnMut(f32)) -> Option<Vec<u8>> {
    use windows_sys::Win32::Networking::WinHttp::*;
    let w = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (agent, host, verb, path) = (w(concat!("Octet/", env!("CARGO_PKG_VERSION"))), w(host), w("GET"), w(path));
    unsafe {
        let session = WinHttpOpen(agent.as_ptr(), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, std::ptr::null(), std::ptr::null(), 0);
        if session.is_null() {
            return None;
        }
        WinHttpSetTimeouts(session, 6000, 6000, 6000, 6000);
        let conn = WinHttpConnect(session, host.as_ptr(), INTERNET_DEFAULT_HTTPS_PORT, 0);
        let req = if conn.is_null() { std::ptr::null_mut() } else {
            WinHttpOpenRequest(conn, verb.as_ptr(), path.as_ptr(), std::ptr::null(), std::ptr::null(), std::ptr::null(), WINHTTP_FLAG_SECURE)
        };
        let mut body = None;
        if !req.is_null() && WinHttpSendRequest(req, std::ptr::null(), 0, std::ptr::null(), 0, 0, 0) != 0 && WinHttpReceiveResponse(req, std::ptr::null_mut()) != 0 {
            let (mut status, mut len) = (0u32, 4u32);
            WinHttpQueryHeaders(req, WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER, std::ptr::null(), (&mut status as *mut u32).cast(), &mut len, std::ptr::null_mut());
            if status == 200 {
                let (mut size, mut len) = (0u32, 4u32);
                WinHttpQueryHeaders(req, WINHTTP_QUERY_CONTENT_LENGTH | WINHTTP_QUERY_FLAG_NUMBER, std::ptr::null(), (&mut size as *mut u32).cast(), &mut len, std::ptr::null_mut());
                let (mut out, mut buf) = (Vec::new(), [0u8; 65536]);
                loop {
                    let mut n = 0u32;
                    if WinHttpReadData(req, buf.as_mut_ptr().cast(), buf.len() as u32, &mut n) == 0 { out.clear(); break; }
                    if n == 0 { body = Some(out); break; }
                    out.extend_from_slice(&buf[..n as usize]);
                    if size > 0 { progress(out.len() as f32 / size as f32); }
                }
            }
        }
        for h in [req, conn, session] {
            if !h.is_null() { WinHttpCloseHandle(h); }
        }
        body
    }
}

/// Gives up the DHCP lease (`renew` false) or asks for one (true), like ipconfig /release and
/// /renew but in-process. Needs administrator rights; renew waits for the DHCP server.
#[cfg(windows)]
pub fn dhcp(index: u32, renew: bool) -> Result<(), String> {
    use windows_sys::Win32::NetworkManagement::IpHelper::{GetInterfaceInfo, IP_ADAPTER_INDEX_MAP, IP_INTERFACE_INFO, IpReleaseAddress, IpRenewAddress};
    let mut size = 0u32;
    unsafe { GetInterfaceInfo(std::ptr::null_mut(), &mut size) };
    let mut buf = vec![0u64; size as usize / 8 + 1];   // u64 for the struct's alignment
    if unsafe { GetInterfaceInfo(buf.as_mut_ptr().cast(), &mut size) } != 0 {
        return Err("Could not list the adapters".into());
    }
    let info = buf.as_ptr() as *const IP_INTERFACE_INFO;
    let maps = unsafe { std::slice::from_raw_parts((*info).Adapter.as_ptr(), (*info).NumAdapters.max(0) as usize) };
    let map: &IP_ADAPTER_INDEX_MAP = maps.iter().find(|m| m.Index == index).ok_or("That adapter has no DHCP to release or renew")?;
    match unsafe { if renew { IpRenewAddress(map) } else { IpReleaseAddress(map) } } {
        0 => Ok(()),
        5 => Err("This needs administrator rights".into()),
        121 | 1460 => Err("No DHCP server answered".into()),   // semaphore timeout / timeout
        e => Err(format!("Windows could not {} the lease (error {e})", if renew { "renew" } else { "release" })),
    }
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

/// Sets an interface's IPv4 DNS servers ("1.1.1.1,8.8.8.8"; empty = from DHCP) and whether it
/// registers its address in DNS. The same call Windows Settings makes; it is kept across restarts.
#[cfg(windows)]
pub fn set_dns(index: u32, servers: &str, register: bool) -> Result<(), String> {
    use windows_sys::Win32::NetworkManagement::IpHelper::*;
    use windows_sys::Win32::NetworkManagement::Ndis::NET_LUID_LH;
    unsafe {
        let mut luid: NET_LUID_LH = std::mem::zeroed();
        let mut guid: windows_sys::core::GUID = std::mem::zeroed();
        if ConvertInterfaceIndexToLuid(index, &mut luid) != 0 || ConvertInterfaceLuidToGuid(&luid, &mut guid) != 0 {
            return Err("That adapter is gone".into());
        }
        let mut ns: Vec<u16> = servers.encode_utf16().chain([0]).collect();
        let s = DNS_INTERFACE_SETTINGS {
            Version: DNS_INTERFACE_SETTINGS_VERSION1,
            Flags: u64::from(DNS_SETTING_NAMESERVER | DNS_SETTING_REGISTRATION_ENABLED | DNS_SETTING_REGISTER_ADAPTER_NAME),
            NameServer: ns.as_mut_ptr(),
            RegistrationEnabled: register.into(),
            RegisterAdapterName: 0,   // "primary only", as before
            ..Default::default()
        };
        match SetInterfaceDnsSettings(guid, &s) {
            0 => Ok(()),
            5 => Err("Changing DNS needs administrator rights".into()),
            e => Err(format!("Windows refused the DNS servers (error {e})")),
        }
    }
}
#[cfg(not(windows))]
pub fn set_dns(_: u32, _: &str, _: bool) -> Result<(), String> {
    Err("Octet only changes settings on Windows".into())
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
/// macOS keeps the cache in mDNSResponder, which only root may tell to drop it: one password prompt.
#[cfg(target_os = "macos")]
pub fn flush_dns() -> bool {
    crate::change::elevate(&crate::change::Request::FlushDns).is_ok()
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
        // A loopback target (a local DNS proxy such as WARP's 127.0.2.2) never answers a ping sent from the adapter's address.
        let src = if ip.is_loopback() { 0 } else { from.map_or(0, ip_u32) };
        let n = IcmpSendEcho2Ex(h, std::ptr::null_mut(), None, std::ptr::null(), src, ip_u32(ip), data.as_ptr().cast(), data.len() as u16,
            std::ptr::null(), reply.as_mut_ptr().cast(), reply.len() as u32, timeout_ms);
        IcmpCloseHandle(h);
        // ICMP_ECHO_REPLY: Address u32, Status u32, RoundTripTime u32, ...
        let status = u32::from_ne_bytes(reply[4..8].try_into().ok()?);
        let rtt = u32::from_ne_bytes(reply[8..12].try_into().ok()?);
        (n > 0 && status == 0).then_some(rtt)
    }
}

/// What a TCP "ping" found.
#[derive(serde::Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Tcp {
    /// The port accepted, after this many ms (the connect's round trip).
    Open(u32),
    /// The device refused the port after this many ms: it is there, but nothing listens.
    Closed(u32),
}

/// "Ping" a TCP port, for devices that drop ICMP. None when nothing answers.
/// Like `ping`, `from` sends it out through that adapter; nothing is sent once connected.
#[cfg(windows)]
pub fn tcping(ip: Ipv4Addr, port: u16, from: Option<Ipv4Addr>, timeout_ms: u32) -> Option<Tcp> {
    use windows_sys::Win32::Networking::WinSock::{
        AF_INET, FD_SET, FIONBIO, INVALID_SOCKET, IPPROTO_TCP, SOCK_STREAM, SOCKADDR, SOCKADDR_IN, TIMEVAL, WSADATA,
        SOL_SOCKET, SO_ERROR, WSAECONNREFUSED, WSAStartup, bind, closesocket, connect, getsockopt, ioctlsocket, select, socket, WSAIoctl,
    };
    let addr = |ip: Ipv4Addr, port: u16| {
        let mut sa: SOCKADDR_IN = unsafe { std::mem::zeroed() };
        sa.sin_family = AF_INET;
        sa.sin_port = port.to_be();
        sa.sin_addr.S_un.S_addr = ip_u32(ip);
        sa
    };
    unsafe {
        let mut wsa: WSADATA = std::mem::zeroed();
        WSAStartup(0x0202, &mut wsa);
        let s = socket(AF_INET as i32, SOCK_STREAM, IPPROTO_TCP);
        if s == INVALID_SOCKET {
            return None;
        }
        let len = size_of::<SOCKADDR_IN>() as i32;
        // A loopback target (a local proxy) only answers from loopback, as with ping.
        if let (Some(src), false) = (from, ip.is_loopback()) {
            let sa = addr(src, 0);
            bind(s, (&sa as *const SOCKADDR_IN).cast::<SOCKADDR>(), len);
        }
        let mut on = 1u32;
        ioctlsocket(s, FIONBIO, &mut on);
        // No SYN retransmissions (SIO_TCP_INITIAL_RTO): otherwise Windows retries a refused connect for 2 s
        // before it says so. Now a refusal comes back at once, and silence ends at the timeout.
        let rto: [u8; 4] = { let ms = timeout_ms.min(65535) as u16; let b = ms.to_ne_bytes(); [b[0], b[1], 0xFE, 0] };   // Rtt, MaxSynRetransmissions = none
        let mut got = 0u32;
        WSAIoctl(s, 0x9800_0011, rto.as_ptr().cast(), 4, std::ptr::null_mut(), 0, &mut got, std::ptr::null_mut(), None);
        let sa = addr(ip, port);
        let t0 = std::time::Instant::now();
        connect(s, (&sa as *const SOCKADDR_IN).cast::<SOCKADDR>(), len);   // WSAEWOULDBLOCK: the answer comes to select
        // Windows reports an accepted connect as writable, a refused one as an exception.
        let mut w: FD_SET = std::mem::zeroed();
        let mut e: FD_SET = std::mem::zeroed();
        (w.fd_count, w.fd_array[0], e.fd_count, e.fd_array[0]) = (1, s, 1, s);
        let tv = TIMEVAL { tv_sec: (timeout_ms / 1000) as i32, tv_usec: (timeout_ms % 1000 * 1000) as i32 };
        let n = select(0, std::ptr::null_mut(), &mut w, &mut e, &tv);
        let ms = t0.elapsed().as_millis() as u32;
        // A failed connect is either refused (someone is there) or timed out (nobody).
        let (mut err, mut n_err) = (0i32, 4i32);
        getsockopt(s, SOL_SOCKET, SO_ERROR, (&mut err as *mut i32).cast(), &mut n_err);
        closesocket(s);
        match (n > 0, w.fd_count > 0) {
            (true, true) => Some(Tcp::Open(ms)),
            (true, false) if err == WSAECONNREFUSED => Some(Tcp::Closed(ms)),
            _ => None,
        }
    }
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

#[cfg(test)]
mod tests {
    #[test]
    fn loopback_answers_ping() {
        assert!(super::ping(std::net::Ipv4Addr::LOCALHOST, None, 1000).is_some());
        assert!(super::ping(std::net::Ipv4Addr::LOCALHOST, Some(std::net::Ipv4Addr::LOCALHOST), 1000).is_some());
    }

    #[test]
    fn tcping_counts_open_and_refused_ports() {
        use super::tcping;
        use std::net::Ipv4Addr;
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let open = l.local_addr().unwrap().port();
        let lo = Ipv4Addr::LOCALHOST;
        assert!(matches!(tcping(lo, open, None, 1500), Some(super::Tcp::Open(_))), "an open port answers");
        drop(l);
        assert!(matches!(tcping(lo, open, None, 1500), Some(super::Tcp::Closed(_))), "a refused port: the device is there");
        assert!(tcping(Ipv4Addr::new(192, 0, 2, 1), 80, None, 300).is_none(), "TEST-NET-1 has nobody");
    }

    #[test]
    fn scan_finds_loopback_and_skips_nobody() {
        let lo = super::scan_host(std::net::Ipv4Addr::LOCALHOST, None, 1).expect("loopback answers");
        assert!(lo.ms.is_some());
        // TEST-NET-1 is never assigned, so nothing answers and nothing is cached.
        assert_eq!(super::scan_host(std::net::Ipv4Addr::new(192, 0, 2, 77), None, 1), None);
    }

    #[test]
    fn names_this_computer() {
        assert!(!super::computer_name().is_empty());
    }

    /// Only made-up interfaces: these must never change a real adapter.
    #[test]
    #[cfg(windows)]
    fn refuses_a_missing_adapter() {
        assert_eq!(super::set_dns(999_999, "1.1.1.1", false), Err("That adapter is gone".into()));
        assert!(super::dhcp(999_999, true).is_err());
    }

    #[test]
    #[ignore]
    fn asks_ipinfo() {
        println!("{:?}", super::public_ip());
    }

    #[test]
    #[cfg(windows)]
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
