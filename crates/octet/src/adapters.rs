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
    gateway = @($c.DefaultIPGateway | Where-Object { $_ })
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
    /// Advanced, sent by the UI only (the reader leaves them empty; the UI fills them from `extra_ips`).
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
    /// For Info: the connection's DNS suffix, IPv6 addresses, the DHCP lease (ms since 1970, 0 = none), Wi-Fi details.
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
    let output = crate::hidden("powershell.exe")
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
                extra_gws: v4(&r.gateway).into_iter().skip(1).collect(),
                suffix: r.suffix,
                ipv6: r.ipv6,
                lease_got: r.lease_got,
                lease_ends: r.lease_ends,
                wifi: r.wifi,
                cfg: Cfg {
                    ip_auto: r.dhcp,
                    address,
                    subnet,
                    gateway: v4(&r.gateway).into_iter().next().unwrap_or_default(),
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

    /// GetAdaptersAddresses for one address family, as a buffer of IP_ADAPTER_ADDRESSES_LH.
    fn gaa(family: u16) -> Result<Vec<u64>, String> {
        let flags = GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_INCLUDE_ALL_INTERFACES | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST;
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

    /// IPv6 addresses by adapter GUID (Info shows them; nothing edits them).
    fn ipv6s() -> HashMap<String, Vec<String>> {
        let mut out: HashMap<String, Vec<String>> = HashMap::new();
        let Ok(buf) = gaa(AF_INET6) else { return out };
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
    fn live() -> Result<HashMap<String, Raw>, String> {
        let buf = gaa(AF_INET)?;
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

    /// What a joined Wi-Fi card is on: network name, 802.11 standard, channel frequency.
    #[derive(Default)]
    struct Wlan {
        ssid: String,
        phy: String,
        mhz: u32,
        wifi: super::Wifi,
    }

    /// The network each Wi-Fi card is joined to, by adapter GUID. Empty when there is no
    /// Wi-Fi (the WLAN service is not running) or nothing is joined.
    #[allow(non_upper_case_globals)]   // Windows names these dot11_phy_type_*
    fn wlans() -> HashMap<String, Wlan> {
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
                    if WlanQueryInterface(h, &info.InterfaceGuid, wlan_intf_opcode_current_connection, std::ptr::null(),
                        &mut size, &mut data, std::ptr::null_mut()) != 0 {
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
                    let mut bss: *mut WLAN_BSS_LIST = std::ptr::null_mut();
                    if WlanGetNetworkBssList(h, &info.InterfaceGuid, ssid, dot11_BSS_type_infrastructure,
                        conn.wlanSecurityAttributes.bSecurityEnabled, std::ptr::null(), &mut bss) == 0 {
                        let entries = std::slice::from_raw_parts((*bss).wlanBssEntries.as_ptr(), (*bss).dwNumberOfItems as usize);
                        if let Some(e) = entries.iter().find(|e| e.dot11Bssid == assoc.dot11Bssid) {
                            w.mhz = e.ulChCenterFrequency / 1000;   // kHz
                        }
                        WlanFreeMemory(bss.cast());
                    }
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
        let (live, mut wlans, mut v6, metered) = std::thread::scope(|s| {
            let (w, v, m) = (s.spawn(wlans), s.spawn(ipv6s), s.spawn(crate::net::metered));
            (live(), w.join().unwrap_or_default(), v.join().unwrap_or_default(), m.join().unwrap_or_default())
        });
        let mut live = live?;
        let mut out = vec![];
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
            match device(&instance) {
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

    /// Reads this machine: `cargo test -p octet reads_this_machine -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn reads_this_machine() {
        for a in read().unwrap() {
            println!("{a:?}");
        }
    }
}
