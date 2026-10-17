//! Read-only snapshot of the machine's IPv4 adapters. Nothing here changes Windows settings.

use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

// One PowerShell call: Get-NetAdapter for the list (the legacy Win32_NetworkAdapter class
// misses some newer drivers, e.g. Qualcomm Wi-Fi 7), CIM for the IPv4 settings, and the
// per-interface registry key that records manually entered DNS (CIM can't tell).
const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$cfg = @{}
Get-CimInstance Win32_NetworkAdapterConfiguration | ForEach-Object { if ($_.SettingID) { $cfg[$_.SettingID] = $_ } }
$out = foreach ($a in Get-NetAdapter) {
  if ($a.Status -eq 'Not Present') { continue }
  $c = $cfg[$a.InterfaceGuid]
  if (-not $c) { continue }
  $key = 'HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\' + $c.SettingID
  $ns = (Get-ItemProperty -Path $key -Name NameServer -ErrorAction SilentlyContinue).NameServer
  [pscustomobject]@{
    id = $c.SettingID; name = $a.Name; description = $a.InterfaceDescription
    mac = [string]$a.MacAddress; status = [string]$a.Status
    dhcp = [bool]$c.DHCPEnabled; dhcpServer = [string]$c.DHCPServer; register = [bool]$c.FullDNSRegistrationEnabled
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
    status: String,
    dhcp: bool,
    dhcp_server: String,
    register: bool,
    ip: Vec<String>,
    subnet: Vec<String>,
    gateway: Vec<String>,
    dns: Vec<String>,
    static_dns: bool,
}

/// The settings the form edits, in the shape the UI uses (empty string = not set).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Adapter {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub mac: String,
    /// "up", "down" or "disabled"
    pub state: &'static str,
    pub dhcp_server: String,
    pub cfg: Cfg,
}

pub fn read() -> Result<Vec<Adapter>, String> {
    let output = crate::hidden("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    parse(&output.stdout)
}

fn parse(json: &[u8]) -> Result<Vec<Adapter>, String> {
    let raw: Vec<Raw> = serde_json::from_slice(json).map_err(|e| e.to_string())?;
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
                name: r.name,
                desc: r.description,
                mac: r.mac.replace(':', "-"),
                state,
                dhcp_server,
                cfg: Cfg {
                    ip_auto: r.dhcp,
                    address,
                    subnet,
                    gateway: v4(&r.gateway).into_iter().next().unwrap_or_default(),
                    dns_auto: !r.static_dns,
                    dns1: dns.first().cloned().unwrap_or_default(),
                    dns2: dns.get(1).cloned().unwrap_or_default(),
                    register: r.register,
                },
            }
        })
        .collect();
    // Connected adapters first, then by name, so the usual choice is at the top.
    adapters.sort_by(|a, b| (b.state == "up").cmp(&(a.state == "up")).then(a.name.cmp(&b.name)));
    Ok(adapters)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mixed_ipv4_ipv6_and_static_dns() {
        let json = br#"[
          {"id":"{6E892664-8DB8-48F5-9498-D58A5B01E630}","name":"Ethernet 4","description":"Realtek USB GbE","mac":"00:E0:4C:68:04:66","status":"Up","dhcp":true,"dhcpServer":"192.168.50.254","register":true,"ip":["192.168.50.253","fe80::1"],"subnet":["255.255.255.0","64"],"gateway":["192.168.50.254"],"dns":["192.168.50.254"],"staticDns":false},
          {"id":"{9EA71783-728E-4550-B3A4-86D1F39EE5E4}","name":"Lab","description":"USB","mac":"","status":"Disconnected","dhcp":false,"dhcpServer":"","register":false,"ip":["fe80::2","10.0.0.5"],"subnet":["64","255.255.0.0"],"gateway":[],"dns":["1.1.1.1","fe80::9","8.8.8.8"],"staticDns":true},
          {"id":"{0}","name":"BT","description":"Bluetooth","mac":"","status":"Disabled","dhcp":true,"dhcpServer":"255.255.255.255","register":true,"ip":[],"subnet":[],"gateway":[],"dns":[],"staticDns":false}
        ]"#;
        let a = parse(json).unwrap();
        let eth = &a[0];
        assert_eq!((eth.name.as_str(), eth.state, eth.mac.as_str()), ("Ethernet 4", "up", "00-E0-4C-68-04-66"));
        assert_eq!((eth.cfg.address.as_str(), eth.cfg.subnet.as_str()), ("192.168.50.253", "255.255.255.0"));
        assert!(eth.cfg.ip_auto && eth.cfg.dns_auto && eth.dhcp_server == "192.168.50.254");
        let bt = &a[1];
        assert_eq!((bt.state, bt.dhcp_server.as_str(), bt.cfg.address.as_str()), ("disabled", "", ""));
        let lab = &a[2];
        assert_eq!((lab.state, lab.cfg.subnet.as_str()), ("down", "255.255.0.0"));
        assert!(!lab.cfg.ip_auto && !lab.cfg.dns_auto);
        assert_eq!((lab.cfg.dns1.as_str(), lab.cfg.dns2.as_str()), ("1.1.1.1", "8.8.8.8"));
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
