//! Read-only snapshot of the machine's IPv4 adapters (on macOS, its network services).
//! Nothing here changes settings.

use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

// The adapters are read straight from Windows (see `native`), in about a millisecond.
// This PowerShell script was the first reader (~1.6 s, Get-NetAdapter alone ~1.4 s);
// it stays as the reference the tests compare the native reader against.
#[cfg(test)]
const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8   # names are not ASCII on Thai Windows
$cfg = @{}
Get-CimInstance Win32_NetworkAdapterConfiguration | ForEach-Object { if ($_.SettingID) { $cfg[$_.SettingID] = $_ } }
$ipif = @{}
Get-NetIPInterface -AddressFamily IPv4 -ErrorAction SilentlyContinue | ForEach-Object { $ipif[[int]$_.ifIndex] = $_ }
$out = foreach ($a in Get-NetAdapter) {
  if ($a.Status -eq 'Not Present') { continue }
  $c = $cfg[$a.InterfaceGuid]
  if (-not $c) { continue }
  $key = 'HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\' + $c.SettingID
  $ns = (Get-ItemProperty -Path $key -Name NameServer -ErrorAction SilentlyContinue).NameServer
  [pscustomobject]@{
    id = $c.SettingID; index = [int]$a.ifIndex; name = $a.Name; description = $a.InterfaceDescription
    mac = [string]$a.MacAddress; status = [string]$a.Status
    dhcp = [bool]$c.DHCPEnabled; dhcpServer = [string]$c.DHCPServer; register = [bool]$c.FullDNSRegistrationEnabled
    ip = @($c.IPAddress | Where-Object { $_ }); subnet = @($c.IPSubnet | Where-Object { $_ })
    gateway = @($c.DefaultIPGateway | Where-Object { $_ }); gwMetric = @($c.GatewayCostMetric | Where-Object { $null -ne $_ })
    ifMetric = $(if ($ipif[[int]$a.ifIndex].AutomaticMetric -eq 'Disabled') { [int]$ipif[[int]$a.ifIndex].InterfaceMetric } else { 0 })
    dns = @($c.DNSServerSearchOrder | Where-Object { $_ }); staticDns = [bool]$ns
  }
}
ConvertTo-Json -InputObject @($out) -Compress -Depth 3
"#;

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
struct Raw {
    id: String,
    index: u32,
    name: String,
    description: String,
    mac: String,
    status: String,
    dhcp: bool,
    dhcp_server: String,
    register: bool,
    ip: Vec<String>,
    subnet: Vec<String>,
    gateway: Vec<String>,
    dns: Vec<String>,
    static_dns: bool,
    /// Each gateway's metric (same order) and the interface metric; 0 = automatic.
    #[serde(default)]
    gw_metric: Vec<u32>,
    #[serde(default)]
    if_metric: u32,
    #[serde(default)]
    ssid: String,
    #[serde(default)]
    speed: u64,
    #[serde(default)]
    phy: String,
    #[serde(default)]
    mhz: u32,
    #[serde(default)]
    ppp: bool,
    /// An address Windows found another device already using (duplicate address detection): set but not in use.
    #[serde(default)]
    conflict: String,
    #[serde(default)]
    metered: bool,
    #[serde(default)]
    suffix: String,
    #[serde(default)]
    ipv6: Vec<String>,
    #[serde(default)]
    lease_got: u64,
    #[serde(default)]
    lease_ends: u64,
    #[serde(skip)]
    wifi: Option<Wifi>,
}

/// The joined Wi-Fi network, for Info: signal 0-100, the access point's MAC, and how it is secured.
#[derive(Serialize, Debug, Clone, Default)]
pub struct Wifi {
    pub signal: u32,
    pub bssid: String,
    pub security: String,
}

/// The settings the form edits, in the shape the UI uses (empty string = not set).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Cfg {
    pub ip_auto: bool,
    pub address: String,
    pub subnet: String,
    pub gateway: String,
    pub dns_auto: bool,
    pub dns1: String,
    pub dns2: String,
    pub register: bool,
    /// Advanced. The reader fills `gws` (with their metrics) and leaves `extra` empty; the UI fills it from `extra_ips`.
    /// Kept when not empty: the request travels to the elevated copy as JSON.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<Addr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gws: Vec<Gw>,
    /// Metric of the first gateway and of the interface; empty = leave as Windows has it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub gw_metric: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub metric: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Addr {
    pub address: String,
    pub subnet: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Gw {
    pub gateway: String,
    #[serde(default)]
    pub metric: String,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Adapter {
    pub id: String,
    /// Interface index; netsh is addressed by this, not the name.
    pub index: u32,
    pub name: String,
    pub desc: String,
    pub mac: String,
    /// The Wi-Fi network it is joined to; empty for anything else.
    pub ssid: String,
    /// Link: speed in bits/s (0 = unknown or not connected); for Wi-Fi also the 802.11
    /// standard ("802.11be") and the channel's centre frequency in MHz.
    pub speed: u64,
    pub phy: String,
    pub mhz: u32,
    /// A dial-up link (PPPoE, VPN): no DHCP, so no lease to renew.
    pub ppp: bool,
    /// An address set on this adapter that another device on the network already has: Windows keeps it but won't use it.
    pub conflict: String,
    /// Windows counts this connection as metered: a phone hotspot it recognised, or one set as metered in Settings.
    pub metered: bool,
    /// "up", "down" or "disabled"
    pub state: &'static str,
    pub dhcp_server: String,
    /// IPv4 addresses after the first (a static apply would drop them), each with its own mask.
    pub extra_ips: Vec<String>,
    pub extra_subnets: Vec<String>,
    /// Default gateways after the first.
    pub extra_gws: Vec<String>,
    /// For Info: the connection's DNS suffix, IPv6 addresses, the DHCP lease (ms since 1970, 0 = none;
    /// an end of 4294967295000, Windows' 0xFFFFFFFF seconds, is a lease that never ends), Wi-Fi details.
    pub suffix: String,
    pub ipv6: Vec<String>,
    pub lease_got: u64,
    pub lease_ends: u64,
    pub wifi: Option<Wifi>,
    pub cfg: Cfg,
}

pub fn read() -> Result<Vec<Adapter>, String> {
    native::read().map(shape)
}

#[cfg(test)]
fn read_ps() -> Result<Vec<Adapter>, String> {
    let output = crate::hidden(r"WindowsPowerShell\v1.0\powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    parse(&output.stdout)
}

#[cfg(test)]
fn parse(json: &[u8]) -> Result<Vec<Adapter>, String> {
    serde_json::from_slice(json).map(shape).map_err(|e| e.to_string())
}

fn shape(raw: Vec<Raw>) -> Vec<Adapter> {
    let v4 = |values: &[String]| -> Vec<String> {
        values.iter().filter(|v| v.parse::<Ipv4Addr>().is_ok()).cloned().collect()
    };
    let mut adapters: Vec<_> = raw
        .into_iter()
        .map(|r| {
            // IPAddress and IPSubnet are parallel lists that mix IPv4 and IPv6.
            let (address, subnet) = r
                .ip
                .iter()
                .zip(r.subnet.iter().map(Some).chain(std::iter::repeat(None)))
                .find(|(ip, _)| ip.parse::<Ipv4Addr>().is_ok())
                .map(|(ip, s)| (ip.clone(), s.cloned().unwrap_or_default()))
                .unwrap_or_default();
            let dns = v4(&r.dns);
            // Gateways with their metrics (both lists mix IPv4 and IPv6 in the PowerShell reader). A route metric of 0
            // is automatic (netsh's gwmetric=0, the Automatic metric box): the route then costs the interface metric alone.
            let num = |m: u32| if m == 0 { String::new() } else { m.to_string() };
            let gws: Vec<Gw> = r.gateway.iter().zip(r.gw_metric.iter().copied().map(Some).chain(std::iter::repeat(None)))
                .filter(|(g, _)| g.parse::<Ipv4Addr>().is_ok()).map(|(g, m)| Gw { gateway: g.clone(), metric: num(m.unwrap_or(0)) }).collect();
            let pairs: Vec<(String, String)> = r.ip.iter().zip(r.subnet.iter().map(Some).chain(std::iter::repeat(None)))
                .filter(|(ip, _)| ip.parse::<Ipv4Addr>().is_ok()).map(|(ip, s)| (ip.clone(), s.cloned().unwrap_or_default())).skip(1).collect();
            let (extra_ips, extra_subnets) = pairs.into_iter().unzip();
            let state = match r.status.as_str() {
                "Up" => "up",
                "Disabled" => "disabled",
                _ => "down",
            };
            // Npcap/APIPA report 255.255.255.255 when no server answered.
            let dhcp_server = Some(r.dhcp_server)
                .filter(|s| r.dhcp && s.parse::<Ipv4Addr>().is_ok_and(|ip| !ip.is_broadcast()))
                .unwrap_or_default();
            // On APIPA (169.254) no DHCP server answered: the lease Windows still keeps is an old one.
            let apipa = r.dhcp && address.parse::<Ipv4Addr>().is_ok_and(|a| a.is_link_local());
            Adapter {
                id: r.id,
                index: r.index,
                name: r.name,
                desc: r.description,
                mac: r.mac.replace(':', "-"),
                ssid: r.ssid,
                speed: r.speed,
                phy: r.phy,
                mhz: r.mhz,
                ppp: r.ppp,
                conflict: r.conflict,
                metered: r.metered,
                state,
                dhcp_server,
                extra_ips,
                extra_subnets,
                extra_gws: gws.iter().skip(1).map(|g| g.gateway.clone()).collect(),
                suffix: r.suffix,
                ipv6: r.ipv6,
                lease_got: if apipa { 0 } else { r.lease_got },
                lease_ends: if apipa { 0 } else { r.lease_ends },
                wifi: r.wifi,
                cfg: Cfg {
                    ip_auto: r.dhcp,
                    address,
                    subnet,
                    gateway: gws.first().map(|g| g.gateway.clone()).unwrap_or_default(),
                    gw_metric: gws.first().map(|g| g.metric.clone()).unwrap_or_default(),
                    gws: gws.into_iter().skip(1).collect(),
                    metric: num(r.if_metric),
                    dns_auto: !r.static_dns,
                    dns1: dns.first().cloned().unwrap_or_default(),
                    dns2: dns.get(1).cloned().unwrap_or_default(),
                    register: r.register,
                    ..Default::default()
                },
            }
        })
        .collect();
    // The adapter that reaches the internet (connected, has a gateway) first, then other
    // connected ones, then disconnected, disabled last; by name within each.
    let rank = |a: &Adapter| (a.state != "up", a.cfg.gateway.is_empty(), a.state == "disabled");
    adapters.sort_by(|a, b| rank(a).cmp(&rank(b)).then(a.name.cmp(&b.name)));
    adapters
}

/// Win32 reader. The list is what Device Manager has as network adapters with a
/// connection name (what Get-NetAdapter shows). Live IPv4 settings come from
/// GetAdaptersAddresses; a disabled adapter has none, so its saved ones come from the registry.
#[cfg(windows)]
mod native {
    use super::Raw;
    use std::collections::HashMap;
    use std::net::Ipv4Addr;
    use windows_sys::Win32::Devices::DeviceAndDriverInstallation::*;
    use windows_sys::Win32::NetworkManagement::IpHelper::*;
    use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6, SOCKADDR_IN, SOCKADDR_IN6, SOCKET_ADDRESS};
    use windows_sys::Win32::System::Registry::*;

    const NET_CLASS: &str = r"SYSTEM\CurrentControlSet\Control\Network\{4D36E972-E325-11CE-BFC1-08002BE10318}";
    const TCPIP: &str = r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces";
    const DRIVERS: &str = r"SYSTEM\CurrentControlSet\Control\Class\{4D36E972-E325-11CE-BFC1-08002BE10318}";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }
    unsafe fn from_wide(p: *const u16) -> String {
        if p.is_null() {
            return String::new();
        }
        let len = (0..).take_while(|&i| unsafe { *p.add(i) } != 0).count();
        String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, len) })
    }
    unsafe fn ipv4(a: &SOCKET_ADDRESS) -> Option<Ipv4Addr> {
        if a.lpSockaddr.is_null() || unsafe { (*a.lpSockaddr).sa_family } != AF_INET {
            return None;
        }
        let sin = unsafe { &*(a.lpSockaddr as *const SOCKADDR_IN) };
        Some(Ipv4Addr::from(unsafe { sin.sin_addr.S_un.S_addr }.to_ne_bytes()))
    }
    fn mask(prefix: u8) -> String {
        Ipv4Addr::from(u32::MAX.checked_shl(32 - prefix as u32).unwrap_or(0)).to_string()
    }

    /// A REG_SZ or REG_MULTI_SZ value (multi-strings joined with ',').
    fn reg_text(key: &str, value: &str) -> Option<String> {
        let (k, v) = (wide(key), wide(value));
        let mut buf = [0u16; 1024];
        let mut len = (buf.len() * 2) as u32;
        let ok = unsafe {
            RegGetValueW(HKEY_LOCAL_MACHINE, k.as_ptr(), v.as_ptr(), RRF_RT_REG_SZ | RRF_RT_REG_MULTI_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut len)
        };
        if ok != 0 {
            return None;
        }
        let text = String::from_utf16_lossy(&buf[..len as usize / 2]);
        Some(text.split('\0').filter(|s| !s.is_empty()).collect::<Vec<_>>().join(","))
    }
    fn reg_dword(key: &str, value: &str) -> Option<u32> {
        let (k, v) = (wide(key), wide(value));
        let (mut d, mut len) = (0u32, 4u32);
        let ok = unsafe {
            RegGetValueW(HKEY_LOCAL_MACHINE, k.as_ptr(), v.as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(), (&mut d as *mut u32).cast(), &mut len)
        };
        (ok == 0).then_some(d)
    }
    fn subkeys(key: &str) -> Vec<String> {
        let k = wide(key);
        let mut h: HKEY = std::ptr::null_mut();
        if unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, k.as_ptr(), 0, KEY_READ, &mut h) } != 0 {
            return vec![];
        }
        let mut out = vec![];
        for i in 0.. {
            let mut name = [0u16; 256];
            let mut len = name.len() as u32;
            let r = unsafe {
                RegEnumKeyExW(h, i, name.as_mut_ptr(), &mut len, std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut())
            };
            if r != 0 {
                break;
            }
            out.push(String::from_utf16_lossy(&name[..len as usize]));
        }
        unsafe { RegCloseKey(h) };
        out
    }

    enum Device {
        Missing,
        Disabled(String),
        Present,
    }
    /// Device Manager's view: unplugged (skip it), disabled (with its description), or present.
    fn device(instance: &str) -> Device {
        let id = wide(instance);
        let mut node = 0u32;
        if unsafe { CM_Locate_DevNodeW(&mut node, id.as_ptr(), CM_LOCATE_DEVNODE_NORMAL) } != CR_SUCCESS {
            return Device::Missing;
        }
        let (mut status, mut problem) = (0u32, 0u32);
        if unsafe { CM_Get_DevNode_Status(&mut status, &mut problem, node, 0) } != CR_SUCCESS {
            return Device::Missing;
        }
        if status & DN_HAS_PROBLEM == 0 || problem != CM_PROB_DISABLED {
            return Device::Present;
        }
        let desc = [CM_DRP_FRIENDLYNAME, CM_DRP_DEVICEDESC].into_iter().find_map(|prop| {
            let mut buf = [0u16; 512];
            let mut len = (buf.len() * 2) as u32;
            let r = unsafe { CM_Get_DevNode_Registry_PropertyW(node, prop, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut len, 0) };
            (r == CR_SUCCESS).then(|| String::from_utf16_lossy(&buf[..len as usize / 2]).trim_end_matches('\0').to_string())
        });
        Device::Disabled(desc.unwrap_or_default())
    }

    /// GetAdaptersAddresses for one address family with the caller's flags, as a buffer of IP_ADAPTER_ADDRESSES_LH.
    fn gaa(family: u16, flags: GET_ADAPTERS_ADDRESSES_FLAGS) -> Result<Vec<u64>, String> {
        let mut size = 16 * 1024u32;
        let mut buf: Vec<u64> = Vec::new(); // u64 keeps the 8-byte alignment the structs need
        for _ in 0..3 {
            buf.resize(size as usize / 8 + 1, 0);
            let r = unsafe { GetAdaptersAddresses(family as u32, flags, std::ptr::null(), buf.as_mut_ptr().cast(), &mut size) };
            if r == 0 {
                return Ok(buf);
            }
            if r != 111 {
                // 111 = buffer too small; size now holds what it needs
                return Err(format!("GetAdaptersAddresses failed ({r})"));
            }
        }
        Err("GetAdaptersAddresses kept growing".into())
    }

    /// What ipv6s() asks for: unicast addresses only, which makes the call ~20x faster.
    /// No INCLUDE_ALL_INTERFACES: an interface listed only with it has no IPv6 binding, so no IPv6 address.
    pub(super) const V6_FLAGS: GET_ADAPTERS_ADDRESSES_FLAGS = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST
        | GAA_FLAG_SKIP_DNS_SERVER | GAA_FLAG_SKIP_DNS_INFO | GAA_FLAG_SKIP_FRIENDLY_NAME;

    const V4_FULL: GET_ADAPTERS_ADDRESSES_FLAGS = GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_INCLUDE_ALL_INTERFACES | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST;
    // Without ALL_INTERFACES Windows lists only interfaces bound to IPv4 (no NDIS filter modules, WAN miniports, tunnels, unplugged cards).
    const V4_LEAN: GET_ADAPTERS_ADDRESSES_FLAGS = V4_FULL & !GAA_FLAG_INCLUDE_ALL_INTERFACES;
    // Tests set this to make read() take the full IPv4 list, as it did before V4_LEAN.
    #[cfg(test)]
    thread_local! { pub(super) static FULL_ONLY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

    /// IPv6 addresses by adapter GUID (Info shows them; nothing edits them).
    pub(super) fn ipv6s(flags: GET_ADAPTERS_ADDRESSES_FLAGS) -> HashMap<String, Vec<String>> {
        let mut out: HashMap<String, Vec<String>> = HashMap::new();
        let Ok(buf) = gaa(AF_INET6, flags) else { return out };
        let mut p = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
        while !p.is_null() {
            let a = unsafe { &*p };
            p = a.Next;
            let id = unsafe { std::ffi::CStr::from_ptr(a.AdapterName as *const _) }.to_string_lossy().to_uppercase();
            let mut u = a.FirstUnicastAddress;
            while !u.is_null() {
                let x = unsafe { &*u };
                let sa = x.Address.lpSockaddr;
                if !sa.is_null() && unsafe { (*sa).sa_family } == AF_INET6 {
                    let sin6 = unsafe { &*(sa as *const SOCKADDR_IN6) };
                    out.entry(id.clone()).or_default().push(std::net::Ipv6Addr::from(unsafe { sin6.sin6_addr.u.Byte }).to_string());
                }
                u = x.Next;
            }
        }
        out
    }

    /// Live IPv4 settings of every interface the IP stack knows, by adapter GUID.
    fn live(flags: GET_ADAPTERS_ADDRESSES_FLAGS) -> Result<HashMap<String, Raw>, String> {
        let buf = gaa(AF_INET, flags)?;
        let mut out = HashMap::new();
        let mut p = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
        while !p.is_null() {
            let a = unsafe { &*p };
            p = a.Next;
            let id = unsafe { std::ffi::CStr::from_ptr(a.AdapterName as *const _) }.to_string_lossy().to_uppercase();
            let flags = unsafe { a.Anonymous2.Flags };
            let len = (a.PhysicalAddressLength as usize).min(8);
            let mut raw = Raw {
                id: id.clone(),
                index: unsafe { a.Anonymous1.Anonymous.IfIndex },
                description: unsafe { from_wide(a.Description) },
                mac: a.PhysicalAddress[..len].iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join("-"),
                status: if a.OperStatus == IfOperStatusUp { "Up" } else { "Disconnected" }.into(),
                dhcp: flags & IP_ADAPTER_DHCP_ENABLED != 0,
                dhcp_server: unsafe { ipv4(&a.Dhcpv4Server) }.map(|ip| ip.to_string()).unwrap_or_default(),
                register: flags & IP_ADAPTER_DDNS_ENABLED != 0,
                // u64::MAX means the driver doesn't say.
                speed: Some(a.TransmitLinkSpeed).filter(|&s| a.OperStatus == IfOperStatusUp && s != u64::MAX).unwrap_or(0),
                ppp: a.IfType == 23,   // IF_TYPE_PPP
                suffix: unsafe { from_wide(a.DnsSuffix) },
                ..Default::default()
            };
            let mut u = a.FirstUnicastAddress;
            while !u.is_null() {
                let x = unsafe { &*u };
                if let Some(ip) = unsafe { ipv4(&x.Address) } {
                    // Duplicate address detection failed: another device has it. It stays the main address (it is
                    // what's set, and the form must show it), with the conflict flagged.
                    if x.DadState == 2 {   // IpDadStateDuplicate
                        raw.conflict = ip.to_string();
                        raw.ip.insert(0, ip.to_string());
                        raw.subnet.insert(0, mask(x.OnLinkPrefixLength));
                        u = x.Next;
                        continue;
                    }
                    // The leased address is the main one, wherever Windows lists it: a typed-in extra can
                    // come first, and the leased one would then be offered for deletion (which netsh refuses).
                    let at = if x.PrefixOrigin == 3 { 0 } else { raw.ip.len() };   // IpPrefixOriginDhcp
                    raw.ip.insert(at, ip.to_string());
                    raw.subnet.insert(at, mask(x.OnLinkPrefixLength));
                }
                u = x.Next;
            }
            let mut g = a.FirstGatewayAddress;
            while !g.is_null() {
                let x = unsafe { &*g };
                raw.gateway.extend(unsafe { ipv4(&x.Address) }.map(|ip| ip.to_string()));
                g = x.Next;
            }
            let mut d = a.FirstDnsServerAddress;
            while !d.is_null() {
                let x = unsafe { &*d };
                raw.dns.extend(unsafe { ipv4(&x.Address) }.map(|ip| ip.to_string()));
                d = x.Next;
            }
            out.insert(id, raw);
        }
        Ok(out)
    }

    /// Metrics: of each default route by (interface index, gateway), and of each interface whose metric
    /// was set by hand (automatic ones are left out), by interface index.
    fn metrics() -> (HashMap<(u32, Ipv4Addr), u32>, HashMap<u32, u32>) {
        let (mut routes, mut ifs) = (HashMap::new(), HashMap::new());
        unsafe {
            let mut t: *mut MIB_IPFORWARD_TABLE2 = std::ptr::null_mut();
            if GetIpForwardTable2(AF_INET, &mut t) == 0 {
                for r in std::slice::from_raw_parts((*t).Table.as_ptr(), (*t).NumEntries as usize) {
                    if r.DestinationPrefix.PrefixLength == 0 {   // 0.0.0.0/0
                        routes.insert((r.InterfaceIndex, Ipv4Addr::from(r.NextHop.Ipv4.sin_addr.S_un.S_addr.to_ne_bytes())), r.Metric);
                    }
                }
                FreeMibTable(t.cast());
            }
            let mut t: *mut MIB_IPINTERFACE_TABLE = std::ptr::null_mut();
            if GetIpInterfaceTable(AF_INET, &mut t) == 0 {
                for r in std::slice::from_raw_parts((*t).Table.as_ptr(), (*t).NumEntries as usize) {
                    if !r.UseAutomaticMetric {
                        ifs.insert(r.InterfaceIndex, r.Metric);
                    }
                }
                FreeMibTable(t.cast());
            }
        }
        (routes, ifs)
    }

    /// What a joined Wi-Fi card is on: network name, 802.11 standard, channel frequency.
    #[derive(Default)]
    pub(super) struct Wlan {
        pub(super) ssid: String,
        pub(super) phy: String,
        pub(super) mhz: u32,
        pub(super) wifi: super::Wifi,
    }

    /// The network each Wi-Fi card is joined to, by adapter GUID. Empty when there is no
    /// Wi-Fi (the WLAN service is not running) or nothing is joined.
    fn wlans() -> HashMap<String, Wlan> {
        wlans_with(true)
    }

    #[cfg(test)]
    pub(super) static FAST_HITS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    /// The frequency as it has always been read: the scan list for our SSID, type and security,
    /// then the entry with our BSSID. 0 when there is none.
    unsafe fn mhz_exact(h: windows_sys::Win32::Foundation::HANDLE, guid: &windows_sys::core::GUID,
        conn: &windows_sys::Win32::NetworkManagement::WiFi::WLAN_CONNECTION_ATTRIBUTES) -> u32 {
        use windows_sys::Win32::NetworkManagement::WiFi::*;
        unsafe {
            let assoc = &conn.wlanAssociationAttributes;
            let mut mhz = 0;
            let mut bss: *mut WLAN_BSS_LIST = std::ptr::null_mut();
            if WlanGetNetworkBssList(h, guid, &assoc.dot11Ssid, dot11_BSS_type_infrastructure,
                conn.wlanSecurityAttributes.bSecurityEnabled, std::ptr::null(), &mut bss) == 0 {
                let entries = std::slice::from_raw_parts((*bss).wlanBssEntries.as_ptr(), (*bss).dwNumberOfItems as usize);
                if let Some(e) = entries.iter().find(|e| e.dot11Bssid == assoc.dot11Bssid) {
                    mhz = e.ulChCenterFrequency / 1000;   // kHz
                }
                WlanFreeMemory(bss.cast());
            }
            mhz
        }
    }

    /// Every entry in the card's scan list (NULL SSID: no filter, no new scan), copied out and
    /// freed here. The copies' IE offsets point nowhere; only the fixed fields are read.
    /// The handle comes as usize: HANDLE is a raw pointer, not Send.
    unsafe fn bss_all(h: usize, guid: windows_sys::core::GUID) -> Option<Vec<windows_sys::Win32::NetworkManagement::WiFi::WLAN_BSS_ENTRY>> {
        use windows_sys::Win32::NetworkManagement::WiFi::*;
        unsafe {
            let mut bss: *mut WLAN_BSS_LIST = std::ptr::null_mut();
            if WlanGetNetworkBssList(h as _, &guid, std::ptr::null(), dot11_BSS_type_any, 0, std::ptr::null(), &mut bss) != 0 {
                return None;
            }
            let v = std::slice::from_raw_parts((*bss).wlanBssEntries.as_ptr(), (*bss).dwNumberOfItems as usize).to_vec();
            WlanFreeMemory(bss.cast());
            Some(v)
        }
    }

    /// fast: fetch the whole scan list while the connection is asked for, instead of after it.
    #[allow(non_upper_case_globals)]   // Windows names these dot11_phy_type_*
    pub(super) fn wlans_with(fast: bool) -> HashMap<String, Wlan> {
        use windows_sys::Win32::NetworkManagement::WiFi::*;
        let mut out = HashMap::new();
        unsafe {
            let (mut ver, mut h) = (0u32, std::ptr::null_mut());
            if WlanOpenHandle(2, std::ptr::null(), &mut ver, &mut h) != 0 {
                return out;
            }
            let mut list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
            if WlanEnumInterfaces(h, std::ptr::null(), &mut list) == 0 {
                let n = (*list).dwNumberOfItems as usize;
                let infos = std::slice::from_raw_parts((*list).InterfaceInfo.as_ptr(), n);
                for info in infos {
                    let (mut size, mut data) = (0u32, std::ptr::null_mut());
                    // Both ask WlanSvc; side by side they end together. The scope joins before the
                    // next card and before the handle is closed.
                    let (hu, guid) = (h as usize, info.InterfaceGuid);
                    let (rc, all) = std::thread::scope(|s| {
                        let t = fast.then(|| s.spawn(move || bss_all(hu, guid)));
                        let rc = WlanQueryInterface(h, &info.InterfaceGuid, wlan_intf_opcode_current_connection, std::ptr::null(),
                            &mut size, &mut data, std::ptr::null_mut());
                        (rc, t.and_then(|t| t.join().ok().flatten()))
                    });
                    if rc != 0 {
                        continue;   // not joined to anything
                    }
                    let conn = &*(data as *const WLAN_CONNECTION_ATTRIBUTES);
                    let assoc = &conn.wlanAssociationAttributes;
                    let ssid = &assoc.dot11Ssid;
                    let mut w = Wlan {
                        ssid: String::from_utf8_lossy(&ssid.ucSSID[..(ssid.uSSIDLength as usize).min(32)]).into_owned(),
                        phy: match assoc.dot11PhyType {
                            dot11_phy_type_ofdm => "802.11a",
                            dot11_phy_type_hrdsss => "802.11b",
                            dot11_phy_type_erp => "802.11g",
                            dot11_phy_type_ht => "802.11n",
                            dot11_phy_type_vht => "802.11ac",
                            dot11_phy_type_dmg => "802.11ad",
                            dot11_phy_type_he => "802.11ax",
                            dot11_phy_type_eht => "802.11be",
                            _ => "",
                        }
                        .into(),
                        mhz: 0,
                        wifi: super::Wifi {
                            signal: assoc.wlanSignalQuality,
                            bssid: assoc.dot11Bssid.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join("-"),
                            security: match conn.wlanSecurityAttributes.dot11AuthAlgorithm {
                                DOT11_AUTH_ALGO_80211_OPEN if conn.wlanSecurityAttributes.bSecurityEnabled == 0 => "Open",
                                DOT11_AUTH_ALGO_80211_OPEN | DOT11_AUTH_ALGO_80211_SHARED_KEY => "WEP",
                                DOT11_AUTH_ALGO_WPA => "WPA-Enterprise",
                                DOT11_AUTH_ALGO_WPA_PSK => "WPA-Personal",
                                DOT11_AUTH_ALGO_RSNA => "WPA2-Enterprise",
                                DOT11_AUTH_ALGO_RSNA_PSK => "WPA2-Personal",
                                DOT11_AUTH_ALGO_WPA3_SAE => "WPA3-Personal",
                                DOT11_AUTH_ALGO_WPA3_ENT_192 | DOT11_AUTH_ALGO_WPA3_ENT => "WPA3-Enterprise",
                                DOT11_AUTH_ALGO_OWE => "Enhanced Open",
                                _ => "",
                            }
                            .into(),
                        },
                    };
                    // The frequency is on the access point's entry in the scan list (the one with our BSSID).
                    // The whole list is used only where it can't disagree with the filtered one: the
                    // auth says whether security is on, and the entry's Privacy bit (0x10) agrees.
                    // ponytail: hidden SSID, OWE, WEP and other auths take the old serial call.
                    let sec = &conn.wlanSecurityAttributes;
                    let privacy = match sec.dot11AuthAlgorithm {
                        DOT11_AUTH_ALGO_80211_OPEN if sec.bSecurityEnabled == 0 => Some(false),
                        DOT11_AUTH_ALGO_WPA | DOT11_AUTH_ALGO_WPA_PSK | DOT11_AUTH_ALGO_RSNA | DOT11_AUTH_ALGO_RSNA_PSK
                        | DOT11_AUTH_ALGO_WPA3_SAE | DOT11_AUTH_ALGO_WPA3_ENT | DOT11_AUTH_ALGO_WPA3_ENT_192
                            if sec.bSecurityEnabled != 0 => Some(true),
                        _ => None,
                    };
                    let n = (ssid.uSSIDLength as usize).min(32);
                    let hit = privacy.and_then(|p| all.as_ref()?.iter().find(|e| {
                        e.dot11Bssid == assoc.dot11Bssid && e.dot11BssType == dot11_BSS_type_infrastructure
                            && e.dot11Ssid.uSSIDLength == ssid.uSSIDLength && e.dot11Ssid.ucSSID[..n] == ssid.ucSSID[..n]
                    }).filter(|e| (e.usCapabilityInformation & 0x10 != 0) == p));
                    w.mhz = match hit {
                        Some(e) => {
                            #[cfg(test)]
                            FAST_HITS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            e.ulChCenterFrequency / 1000   // kHz
                        }
                        None => mhz_exact(h, &info.InterfaceGuid, conn),
                    };
                    WlanFreeMemory(data);
                    let g = info.InterfaceGuid;
                    let id = format!("{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{}}}", g.data1, g.data2, g.data3, g.data4[0], g.data4[1],
                        g.data4[2..].iter().map(|b| format!("{b:02X}")).collect::<String>());
                    out.insert(id, w);
                }
                WlanFreeMemory(list.cast());
            }
            WlanCloseHandle(h, std::ptr::null());
        }
        out
    }

    pub fn read() -> Result<Vec<Raw>, String> {
        // Each asks Windows something that takes some 20 ms: asked side by side, a read takes one of those, not three.
        let (lean, mut wlans, mut v6, metered, (routes, if_metrics)) = std::thread::scope(|s| {
            let (w, v, m, r) = (s.spawn(wlans), s.spawn(|| ipv6s(V6_FLAGS)), s.spawn(crate::net::metered), s.spawn(metrics));
            (live(V4_LEAN), w.join().unwrap_or_default(), v.join().unwrap_or_default(), m.join().unwrap_or_default(), r.join().unwrap_or_default())
        });
        #[cfg(test)]
        let lean = if FULL_ONLY.get() { Err(String::new()) } else { lean };
        let mut entries = vec![];
        // One entry per network adapter driver. Virtual ports of one card (Wi-Fi Direct)
        // have no driver entry of their own, and NCF_HIDDEN (0x8) marks WAN miniports and
        // the kernel debugger: both are left out, as Get-NetAdapter does.
        for sub in subkeys(DRIVERS) {
            let drv = format!(r"{DRIVERS}\{sub}");
            let Some(guid) = reg_text(&drv, "NetCfgInstanceId") else { continue };
            if reg_dword(&drv, "Characteristics").is_some_and(|c| c & 0x8 != 0) {
                continue;
            }
            let conn = format!(r"{NET_CLASS}\{guid}\Connection");
            let (Some(name), Some(instance)) = (reg_text(&conn, "Name"), reg_text(&conn, "PnPInstanceId")) else { continue };
            let id = guid.to_uppercase();
            let tcp = format!(r"{TCPIP}\{id}");
            // Typed-in DNS servers are kept here; DHCP-given ones are not.
            let static_dns = reg_text(&tcp, "NameServer").is_some_and(|s| !s.is_empty());
            entries.push((id, name, tcp, static_dns, device(&instance)));
        }
        // ALL_INTERFACES only adds interfaces with no IPv4 binding: if a present adapter is one of them
        // (e.g. a Hyper-V external-switch card) or the lean call failed, ask again exactly as before.
        // ponytail: on a machine with a present IPv4-unbound adapter every read pays both calls (lean + full, the full one serial after the join); remember which GUIDs need it if that hurts.
        let mut live = match lean {
            Ok(m) if entries.iter().all(|(id, .., d)| !matches!(d, Device::Present) || m.contains_key(id)) => m,
            _ => live(V4_FULL)?,
        };
        let mut out = vec![];
        for (id, name, tcp, static_dns, device) in entries {
            match device {
                Device::Missing => {}
                Device::Present => {
                    // No IPv4 binding means nothing for Octet to edit.
                    let Some(mut raw) = live.remove(&id) else { continue };
                    raw.name = name;
                    raw.static_dns = static_dns;
                    let w = wlans.remove(&id);
                    raw.wifi = w.as_ref().filter(|w| !w.ssid.is_empty()).map(|w| w.wifi.clone());
                    let w = w.unwrap_or_default();
                    (raw.ssid, raw.phy, raw.mhz) = (w.ssid, w.phy, w.mhz);
                    raw.ipv6 = v6.remove(&id).unwrap_or_default();
                    raw.metered = metered.contains(&id);
                    // The lease, in seconds since 1970, as Windows keeps it for the interface.
                    if raw.dhcp && raw.status == "Up" {
                        let s = |v| reg_dword(&tcp, v).map_or(0, |t| u64::from(t) * 1000);
                        (raw.lease_got, raw.lease_ends) = (s("LeaseObtainedTime"), s("LeaseTerminatesTime"));
                    }
                    // Not connected and on DHCP: no lease, so any 169.254 address is leftover noise.
                    if raw.status != "Up" && raw.dhcp {
                        (raw.ip, raw.subnet, raw.gateway) = Default::default();
                        if !static_dns {
                            raw.dns.clear();
                        }
                    }
                    raw.gw_metric = raw.gateway.iter().map(|g| g.parse().ok().and_then(|g| routes.get(&(raw.index, g))).copied().unwrap_or(0)).collect();
                    raw.if_metric = if_metrics.get(&raw.index).copied().unwrap_or(0);
                    out.push(raw);
                }
                Device::Disabled(description) => {
                    // Windows reports nothing live for a disabled adapter, so its settings come from the registry,
                    // where they wait for it to come back. Left blank, the form would show a static adapter as
                    // empty, and an Apply from it would wipe what's there.
                    let dhcp = reg_dword(&tcp, "EnableDHCP").is_none_or(|d| d != 0);
                    let list = |v| reg_text(&tcp, v).map(|t| t.split([',', ' ']).filter(|x| !x.is_empty() && *x != "0.0.0.0").map(String::from).collect::<Vec<_>>()).unwrap_or_default();
                    let (ip, subnet, gateway) = if dhcp { Default::default() } else { (list("IPAddress"), list("SubnetMask"), list("DefaultGateway")) };
                    let (ip, subnet): (Vec<_>, Vec<_>) = ip.into_iter().zip(subnet).unzip();   // read as pairs
                    out.push(Raw {
                        id,
                        name,
                        description,
                        status: "Disabled".into(),
                        static_dns,
                        dhcp,
                        register: reg_dword(&tcp, "RegistrationEnabled").is_none_or(|d| d != 0),
                        ip,
                        subnet,
                        gw_metric: list("DefaultGatewayMetric").iter().map(|m| m.parse().unwrap_or(0)).collect(),
                        gateway,
                        dns: if static_dns { list("NameServer") } else { vec![] },
                        ..Default::default()
                    })
                }
            }
        }
        Ok(out)
    }
}

/// macOS reader: the network services (see mac.rs), with what the device and the Wi-Fi card add.
#[cfg(target_os = "macos")]
mod native {
    use super::{Raw, Wifi};
    use crate::mac;

    pub fn read() -> Result<Vec<Raw>, String> {
        let (names, links) = (mac::display_names(), mac::links());
        Ok(mac::read()?
            .into_iter()
            .map(|s| {
                let dev = s.description;
                let link = links.get(&dev);
                let up = s.status == "Up";
                let w = if s.wifi_card && up { mac::wlan(&dev) } else { None };
                Raw {
                    id: s.id,
                    index: s.index,
                    name: s.name,
                    description: names.get(&dev).map_or(dev.clone(), |n| format!("{n} ({dev})")),
                    mac: link.map(|l| l.mac.clone()).unwrap_or_default(),
                    status: s.status,
                    dhcp: s.dhcp,
                    dhcp_server: s.dhcp_server,
                    register: false,   // a Windows (Active Directory) setting; macOS has none
                    ip: s.ip,
                    subnet: s.subnet,
                    gateway: s.gateway,
                    dns: s.dns,
                    static_dns: s.static_dns,
                    gw_metric: vec![],   // ponytail: macOS gateway and interface metrics not read; Apply there sends none
                    if_metric: 0,
                    ssid: w.as_ref().map(|w| w.ssid.clone()).unwrap_or_default(),
                    speed: w.as_ref().map(|w| w.rate).or(link.map(|l| l.speed)).filter(|_| up).unwrap_or(0),
                    phy: w.as_ref().map(|w| w.phy.clone()).unwrap_or_default(),
                    mhz: w.as_ref().map_or(0, |w| w.mhz),
                    ppp: false,
                    conflict: String::new(),   // ponytail: macOS duplicate-address state not read yet
                    metered: false,   // ponytail: macOS's "Low Data Mode" not read; the view guesses from the address
                    suffix: s.suffix,
                    ipv6: link.map(|l| l.ipv6.clone()).unwrap_or_default(),
                    lease_got: s.lease_got,
                    lease_ends: s.lease_ends,
                    wifi: w.map(|w| Wifi { signal: w.signal, bssid: w.bssid, security: w.security }),
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prints each adapter's Wi-Fi network: cargo test --release -p octet shows_ssids -- --ignored --nocapture
    #[test]
    #[ignore]
    fn shows_ssids() {
        for a in read().unwrap() {
            println!("SSID {:<32} {:?} {} {} MHz {} bit/s", a.name, a.ssid, a.phy, a.mhz, a.speed);
        }
    }

    #[test]
    fn parses_mixed_ipv4_ipv6_and_static_dns() {
        let json = br#"[
          {"id":"{00000000-0000-0000-0000-000000000001}","index":7,"name":"Ethernet 4","description":"Realtek USB GbE","mac":"02:00:00:00:00:01","status":"Up","dhcp":true,"dhcpServer":"192.168.50.254","register":true,"ip":["192.168.50.253","fe80::1"],"subnet":["255.255.255.0","64"],"gateway":["192.168.50.254"],"dns":["192.168.50.254"],"staticDns":false},
          {"id":"{00000000-0000-0000-0000-000000000002}","index":9,"name":"Lab","description":"USB","mac":"","status":"Disconnected","dhcp":false,"dhcpServer":"","register":false,"ip":["fe80::2","10.0.0.5","10.0.0.6"],"subnet":["64","255.255.0.0"],"gateway":[],"dns":["1.1.1.1","fe80::9","8.8.8.8"],"staticDns":true},
          {"id":"{0}","index":3,"name":"BT","description":"Bluetooth","mac":"","status":"Disabled","dhcp":true,"dhcpServer":"255.255.255.255","register":true,"ip":[],"subnet":[],"gateway":[],"dns":[],"staticDns":false}
        ]"#;
        let a = parse(json).unwrap();
        let eth = &a[0];
        assert_eq!((eth.name.as_str(), eth.state, eth.mac.as_str()), ("Ethernet 4", "up", "02-00-00-00-00-01"));
        assert_eq!((eth.cfg.address.as_str(), eth.cfg.subnet.as_str()), ("192.168.50.253", "255.255.255.0"));
        assert!(eth.cfg.ip_auto && eth.cfg.dns_auto && eth.dhcp_server == "192.168.50.254");
        let lab = &a[1];
        assert_eq!((lab.state, lab.index, lab.cfg.subnet.as_str()), ("down", 9, "255.255.0.0"));
        assert_eq!(lab.extra_ips, vec!["10.0.0.6".to_string()]);
        assert_eq!(lab.extra_subnets, vec![String::new()], "a mask the list did not give stays empty");
        let bt = &a[2];
        assert_eq!((bt.state, bt.dhcp_server.as_str(), bt.cfg.address.as_str()), ("disabled", "", ""));
        assert!(!lab.cfg.ip_auto && !lab.cfg.dns_auto);
        assert_eq!((lab.cfg.dns1.as_str(), lab.cfg.dns2.as_str()), ("1.1.1.1", "8.8.8.8"));
    }

    #[test]
    fn gateway_and_interface_metrics_keep_their_place() {
        // As CIM lists them: IPv4 and IPv6 gateways mixed, a metric for each; 0 = automatic.
        let json = br#"[{"id":"{1}","index":4,"name":"LAN","description":"","mac":"","status":"Up","dhcp":false,"dhcpServer":"","register":false,
          "ip":["10.0.0.5"],"subnet":["255.255.255.0"],"gateway":["10.0.0.1","fe80::1","10.0.0.2","10.0.0.3"],"gwMetric":[0,256,25,0],"ifMetric":15,
          "dns":[],"staticDns":true}]"#;
        let a = &parse(json).unwrap()[0];
        assert_eq!((a.cfg.gateway.as_str(), a.cfg.gw_metric.as_str(), a.cfg.metric.as_str()), ("10.0.0.1", "", "15"));
        assert_eq!(a.cfg.gws, [Gw { gateway: "10.0.0.2".into(), metric: "25".into() }, Gw { gateway: "10.0.0.3".into(), metric: String::new() }]);
        assert_eq!(a.extra_gws, ["10.0.0.2", "10.0.0.3"]);
    }

    #[test]
    fn apipa_shows_no_old_lease() {
        let json = |ip: &str| format!(r#"[{{"id":"{{1}}","index":4,"name":"LAN","description":"","mac":"","status":"Up","dhcp":true,"dhcpServer":"","register":false,
          "ip":["{ip}"],"subnet":["255.255.0.0"],"gateway":[],"dns":[],"staticDns":false,"leaseGot":1000,"leaseEnds":2000}}]"#);
        let a = &parse(json("169.254.10.20").as_bytes()).unwrap()[0];
        assert_eq!((a.lease_got, a.lease_ends), (0, 0));
        let a = &parse(json("10.1.2.3").as_bytes()).unwrap()[0];
        assert_eq!((a.lease_got, a.lease_ends), (1000, 2000));
    }

    /// The native reader must agree with the PowerShell one on this machine, field by field,
    /// and the timings of both are printed:
    /// `cargo test --release -p octet native_matches_powershell -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn native_matches_powershell() {
        use std::time::Instant;
        let time = |f: &dyn Fn() -> Vec<Adapter>, runs: u32| {
            let mut ms: Vec<f64> = (0..runs).map(|_| { let t = Instant::now(); f(); t.elapsed().as_secs_f64() * 1000.0 }).collect();
            ms.sort_by(|a, b| a.total_cmp(b));
            (ms[0], ms[ms.len() / 2], ms[ms.len() - 1])
        };
        let (ps, native) = (read_ps().unwrap(), read().unwrap());
        let row = |a: &Adapter| format!("{:?}", (&a.id, a.index, &a.name, &a.desc, &a.mac, a.state, &a.dhcp_server, &a.extra_ips, &a.cfg));
        println!("ADAPTERS ps={} native={}", ps.len(), native.len());
        let mut diffs = 0;
        // Where CIM has nothing for a disconnected adapter (IPEnabled is false), the native
        // reader gives the saved settings instead: Register in DNS and typed-in DNS servers.
        let known = |p: &Adapter, n: &Adapter| {
            let mut p = p.clone();
            if p.state != "up" && p.cfg.dns1.is_empty() && !p.cfg.register {
                (p.cfg.register, p.cfg.dns1, p.cfg.dns2) = (n.cfg.register, n.cfg.dns1.clone(), n.cfg.dns2.clone());
            }
            row(&p) == row(n)
        };
        for p in &ps {
            match native.iter().find(|n| n.id == p.id) {
                Some(n) if row(n) == row(p) => println!("SAME  {}", p.name),
                Some(n) if known(p, n) => println!("SAME  {} (plus saved settings CIM leaves out while disconnected: register={}, dns={:?})", p.name, n.cfg.register, (&n.cfg.dns1, &n.cfg.dns2)),
                Some(n) => { diffs += 1; println!("DIFF  {}\n  ps:     {}\n  native: {}", p.name, row(p), row(n)) }
                None => { diffs += 1; println!("MISSING in native: {}", row(p)) }
            }
        }
        for n in native.iter().filter(|n| !ps.iter().any(|p| p.id == n.id)) {
            diffs += 1;
            println!("EXTRA in native: {}", row(n));
        }
        let order = |v: &[Adapter]| v.iter().map(|a| a.name.clone()).collect::<Vec<_>>();
        println!("ORDER same: {}", order(&ps) == order(&native));
        let (a, b, c) = time(&|| read_ps().unwrap(), 5);
        println!("TIME powershell ms: min {a:.1} median {b:.1} max {c:.1}");
        let (a, b, c) = time(&|| read().unwrap(), 50);
        println!("TIME native     ms: min {a:.2} median {b:.2} max {c:.2}");
        assert_eq!(diffs, 0);
    }

    /// The lean IPv6 flags give the same map as the full ones:
    /// `cargo test --release -p octet lean_ipv6_flags_read_the_same -- --ignored`.
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn lean_ipv6_flags_read_the_same() {
        use windows_sys::Win32::NetworkManagement::IpHelper::*;
        let full = GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_INCLUDE_ALL_INTERFACES | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST;
        assert_eq!(native::ipv6s(full), native::ipv6s(native::V6_FLAGS));
    }

    /// The lean IPv4 list must read the same as the full one it stands in for:
    /// `cargo test --release -p octet lean_ipv4_table_reads_the_same -- --ignored`.
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn lean_ipv4_table_reads_the_same() {
        let lean = format!("{:?}", read());
        native::FULL_ONLY.set(true);
        let full = format!("{:?}", read());
        native::FULL_ONLY.set(false);
        assert_eq!(lean, full);
    }

    /// The parallel scan-list read gives the same Wi-Fi map as the serial one (signal left out:
    /// it moves between any two calls). Proves it only for the network joined while it runs:
    /// `cargo test --release -p octet wifi_parallel_bss_reads_the_same -- --ignored --nocapture`.
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn wifi_parallel_bss_reads_the_same() {
        use std::sync::atomic::Ordering;
        let key = |m: std::collections::HashMap<String, native::Wlan>| {
            m.into_iter().map(|(g, w)| (g, (w.ssid, w.phy, w.mhz, w.wifi.bssid, w.wifi.security))).collect::<std::collections::BTreeMap<_, _>>()
        };
        let first = key(native::wlans_with(false));
        let Some((_, joined)) = first.iter().find(|(_, w)| w.2 != 0) else {
            println!("skipped: no joined Wi-Fi");
            return;
        };
        println!("joined {:?}, auth {:?}", joined.0, joined.4);
        native::FAST_HITS.store(0, Ordering::Relaxed);
        for _ in 0..20 {
            assert_eq!(key(native::wlans_with(true)), key(native::wlans_with(false)));
        }
        println!("fast path taken {} of 20", native::FAST_HITS.load(Ordering::Relaxed));
    }

    /// Reads this machine: `cargo test -p octet reads_this_machine -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn reads_this_machine() {
        for a in read().unwrap() {
            println!("{a:?}");
        }
    }
}
