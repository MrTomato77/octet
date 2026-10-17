//! Read-only snapshot of the machine's IPv4 adapters. Nothing here changes Windows settings.

use serde::Deserialize;
use simple_ip_core::{DnsConfig, IpConfig, NetworkConfig, SampleAdapter, parse_subnet};
use std::net::Ipv4Addr;

// One PowerShell call over local WMI/CIM plus the per-interface registry key that
// records manually entered DNS servers (CIM does not say whether DNS is static).
const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$cfg = @{}
Get-CimInstance Win32_NetworkAdapterConfiguration | ForEach-Object { $cfg[[int]$_.Index] = $_ }
$out = foreach ($a in Get-CimInstance Win32_NetworkAdapter -Filter 'NetConnectionID IS NOT NULL') {
  $c = $cfg[[int]$a.Index]
  if (-not $c -or -not $c.SettingID) { continue }
  $key = 'HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\' + $c.SettingID
  $ns = (Get-ItemProperty -Path $key -Name NameServer -ErrorAction SilentlyContinue).NameServer
  [pscustomobject]@{
    id = $c.SettingID; name = $a.NetConnectionID; description = $a.Description
    mac = [string]$a.MACAddress; connected = ($a.NetConnectionStatus -eq 2)
    dhcp = [bool]$c.DHCPEnabled; dhcpServer = [string]$c.DHCPServer
    ip = @($c.IPAddress | Where-Object { $_ }); subnet = @($c.IPSubnet | Where-Object { $_ })
    gateway = @($c.DefaultIPGateway | Where-Object { $_ })
    dns = @($c.DNSServerSearchOrder | Where-Object { $_ }); staticDns = [bool]$ns
  }
}
ConvertTo-Json -InputObject @($out) -Compress -Depth 3
"#;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    id: String,
    name: String,
    description: String,
    mac: String,
    connected: bool,
    dhcp: bool,
    dhcp_server: String,
    ip: Vec<String>,
    subnet: Vec<String>,
    gateway: Vec<String>,
    dns: Vec<String>,
    static_dns: bool,
}

pub fn read() -> Result<Vec<SampleAdapter>, String> {
    let mut command = std::process::Command::new("powershell.exe");
    command.args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    parse(&output.stdout)
}

fn parse(json: &[u8]) -> Result<Vec<SampleAdapter>, String> {
    let raw: Vec<Raw> = serde_json::from_slice(json).map_err(|e| e.to_string())?;
    let v4 = |values: &[String]| -> Vec<Ipv4Addr> {
        values.iter().filter_map(|v| v.parse().ok()).collect()
    };
    let mut adapters: Vec<_> = raw
        .into_iter()
        .map(|r| {
            // IPAddress and IPSubnet are parallel lists that mix IPv4 and IPv6.
            let (address, prefix) =
                r.ip.iter()
                    .zip(r.subnet.iter().map(Some).chain(std::iter::repeat(None)))
                    .find_map(|(ip, subnet)| {
                        let ip = ip.parse::<Ipv4Addr>().ok()?;
                        let prefix = subnet.and_then(|s| parse_subnet(s).ok()).unwrap_or(32);
                        Some((Some(ip), prefix))
                    })
                    .unwrap_or((None, 0));
            let gateway = v4(&r.gateway).first().copied();
            let dns = v4(&r.dns);
            let ip = match (r.dhcp, address) {
                (false, Some(address)) => IpConfig::Static {
                    address,
                    prefix,
                    gateway,
                },
                _ => IpConfig::Dhcp,
            };
            let dns_config = if r.static_dns {
                DnsConfig::Manual(dns.clone())
            } else {
                DnsConfig::Automatic
            };
            SampleAdapter {
                id: uuid::Uuid::parse_str(r.id.trim_matches(['{', '}'])).unwrap_or_default(),
                name: r.name,
                description: r.description,
                mac: r.mac.replace(':', "-"),
                connected: r.connected,
                address,
                prefix,
                gateway,
                dns,
                // Npcap/APIPA report 255.255.255.255 when no server answered.
                dhcp_server: if r.dhcp {
                    r.dhcp_server
                        .parse()
                        .ok()
                        .filter(|ip: &Ipv4Addr| !ip.is_broadcast())
                } else {
                    None
                },
                config: NetworkConfig {
                    ip,
                    dns: dns_config,
                },
            }
        })
        .collect();
    // Connected adapters first, then by name, so the usual choice is at the top.
    adapters.sort_by(|a, b| b.connected.cmp(&a.connected).then(a.name.cmp(&b.name)));
    Ok(adapters)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mixed_ipv4_ipv6_and_static_dns() {
        let json = br#"[
          {"id":"{6E892664-8DB8-48F5-9498-D58A5B01E630}","name":"Ethernet 4","description":"Realtek USB GbE","mac":"00:E0:4C:68:04:66","connected":true,"dhcp":true,"dhcpServer":"192.168.50.254","ip":["192.168.50.253","fe80::1"],"subnet":["255.255.255.0","64"],"gateway":["192.168.50.254"],"dns":["192.168.50.254"],"staticDns":false},
          {"id":"{9EA71783-728E-4550-B3A4-86D1F39EE5E4}","name":"Lab","description":"USB","mac":"","connected":false,"dhcp":false,"dhcpServer":"","ip":["fe80::2","10.0.0.5"],"subnet":["64","255.255.0.0"],"gateway":[],"dns":["1.1.1.1"],"staticDns":true}
        ]"#;
        let adapters = parse(json).unwrap();
        let eth = &adapters[0];
        assert_eq!(eth.name, "Ethernet 4");
        assert_eq!(eth.mac, "00-E0-4C-68-04-66");
        assert_eq!(eth.address, Some(Ipv4Addr::new(192, 168, 50, 253)));
        assert_eq!(eth.prefix, 24);
        assert_eq!(eth.config.ip, IpConfig::Dhcp);
        assert_eq!(eth.config.dns, DnsConfig::Automatic);
        assert_eq!(eth.dhcp_server, Some(Ipv4Addr::new(192, 168, 50, 254)));
        let lab = &adapters[1];
        assert_eq!(lab.prefix, 16);
        assert!(matches!(lab.config.ip, IpConfig::Static { .. }));
        assert_eq!(
            lab.config.dns,
            DnsConfig::Manual(vec![Ipv4Addr::new(1, 1, 1, 1)])
        );
    }
}

#[cfg(test)]
mod machine {
    /// Reads this machine: `cargo test -p simple-ip-v2 reads_this_machine -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn reads_this_machine() {
        for a in super::read().unwrap() {
            println!(
                "{} | {} | {:?}/{} gw {:?} dns {:?} dhcp {:?} | {:?}",
                a.name, a.connected, a.address, a.prefix, a.gateway, a.dns, a.dhcp_server, a.config
            );
        }
    }
}
