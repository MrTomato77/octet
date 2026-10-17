//! Changes to Windows network settings. The UI asks for a `Request`; it is validated,
//! written to a temp file and carried out by a second copy of Octet started with UAC
//! ("--elevated <file>"), which validates it again and runs netsh/ipconfig directly
//! (no shell, so adapter names cannot inject commands).

use crate::adapters::Cfg;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Request {
    /// `ip` / `dns`: which halves changed. netsh fails on "DHCP is already enabled",
    /// so an unchanged half is left alone.
    Apply { name: String, cfg: Cfg, ip: bool, dns: bool },
    Release { name: String },
    Renew { name: String },
    Enable { name: String, on: bool },
}

fn ipv4(v: &str, what: &str) -> Result<Ipv4Addr, String> {
    v.parse().map_err(|_| format!("{what} is not a valid IPv4 address"))
}

/// The exact programs and arguments a request runs. Validates everything first.
pub fn commands(r: &Request) -> Result<Vec<Vec<String>>, String> {
    let name = match r {
        Request::Apply { name, .. } | Request::Release { name } | Request::Renew { name } | Request::Enable { name, .. } => name,
    };
    if name.is_empty() || name.chars().any(|c| c.is_control() || c == '"') {
        return Err("Adapter name is not usable".into());
    }
    let cmd = |args: &[&str]| args.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let iface = format!("name={name}");
    Ok(match r {
        Request::Release { .. } => vec![cmd(&["ipconfig", "/release", name])],
        Request::Renew { .. } => vec![cmd(&["ipconfig", "/renew", name])],
        Request::Enable { on, .. } => {
            vec![cmd(&["netsh", "interface", "set", "interface", &iface, if *on { "admin=enable" } else { "admin=disable" }])]
        }
        Request::Apply { cfg, ip, dns, .. } => {
            let mut out = vec![];
            if *ip {
                if cfg.ip_auto {
                    out.push(cmd(&["netsh", "interface", "ipv4", "set", "address", &iface, "source=dhcp"]));
                } else {
                    let address = ipv4(&cfg.address, "IP address")?;
                    let prefix = octet_core::parse_subnet(&cfg.subnet).map_err(|e| e.to_string())?;
                    if prefix == 0 {
                        return Err("Subnet mask cannot be 0.0.0.0".into());
                    }
                    let gateway = match cfg.gateway.as_str() {
                        "" => "gateway=none".to_string(),
                        g => format!("gateway={}", ipv4(g, "Gateway")?),
                    };
                    out.push(cmd(&[
                        "netsh", "interface", "ipv4", "set", "address", &iface, "source=static",
                        &format!("address={address}"), &format!("mask={}", octet_core::prefix_mask(prefix)), &gateway,
                    ]));
                }
            }
            if *dns {
                let register = if cfg.register { "register=primary" } else { "register=none" };
                if cfg.dns_auto {
                    out.push(cmd(&["netsh", "interface", "ipv4", "set", "dnsservers", &iface, "source=dhcp", register]));
                } else if cfg.dns1.is_empty() {
                    out.push(cmd(&["netsh", "interface", "ipv4", "set", "dnsservers", &iface, "source=static", "address=none", register]));
                } else {
                    let d1 = ipv4(&cfg.dns1, "Preferred DNS")?;
                    out.push(cmd(&[
                        "netsh", "interface", "ipv4", "set", "dnsservers", &iface, "source=static",
                        &format!("address={d1}"), register, "validate=no",
                    ]));
                    if !cfg.dns2.is_empty() {
                        let d2 = ipv4(&cfg.dns2, "Alternate DNS")?;
                        out.push(cmd(&["netsh", "interface", "ipv4", "add", "dnsservers", &iface, &format!("address={d2}"), "index=2", "validate=no"]));
                    }
                }
            }
            out
        }
    })
}

/// Unelevated side: validate, then start the elevated copy and wait for it.
pub fn elevate(r: &Request) -> Result<(), String> {
    let cmds = commands(r)?;
    if cmds.is_empty() {
        return Ok(());
    }
    let file = std::env::temp_dir().join(format!("octet-{}.json", uuid::Uuid::new_v4()));
    let out = file.with_extension("out");
    std::fs::write(&file, serde_json::to_vec(r).unwrap()).map_err(|e| e.to_string())?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let q = |p: &std::path::Path| p.display().to_string().replace('\'', "''");
    let script = format!(
        "$p = Start-Process -FilePath '{}' -ArgumentList '--elevated','\"{}\"' -Verb RunAs -WindowStyle Hidden -Wait -PassThru; exit $p.ExitCode",
        q(&exe),
        q(&file)
    );
    let status = crate::hidden("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|e| e.to_string());
    let detail = std::fs::read_to_string(&out).unwrap_or_default();
    let _ = std::fs::remove_file(&file);
    let _ = std::fs::remove_file(&out);
    let status = status?;
    if status.status.success() {
        Ok(())
    } else if detail.is_empty() {
        Err("Cancelled: administrator permission was not given".into())
    } else {
        Err(detail.trim().to_string())
    }
}

/// Elevated side: run the request from `path`, write any failure next to it.
pub fn run_elevated(path: &str) -> i32 {
    let out = std::path::Path::new(path).with_extension("out");
    let fail = |msg: String| {
        let _ = std::fs::write(&out, msg);
        1
    };
    let req: Request = match std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()) {
        Some(r) => r,
        None => return fail("Could not read the change request".into()),
    };
    let cmds = match commands(&req) {
        Ok(c) => c,
        Err(e) => return fail(e),
    };
    for c in cmds {
        match crate::hidden(&c[0]).args(&c[1..]).output() {
            Ok(o) if o.status.success() => {}
            Ok(o) => {
                let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
                let text = text.trim();
                return fail(if text.is_empty() { format!("{} failed", c.join(" ")) } else { text.to_string() });
            }
            Err(e) => return fail(e.to_string()),
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Cfg {
        Cfg {
            ip_auto: false,
            address: "192.168.1.50".into(),
            subnet: "255.255.255.0".into(),
            gateway: "192.168.1.1".into(),
            dns_auto: false,
            dns1: "1.1.1.1".into(),
            dns2: "8.8.8.8".into(),
            register: false,
        }
    }

    #[test]
    fn builds_static_apply() {
        let c = commands(&Request::Apply { name: "vEthernet (WSL)".into(), cfg: cfg(), ip: true, dns: true }).unwrap();
        assert_eq!(
            c[0].join(" "),
            "netsh interface ipv4 set address name=vEthernet (WSL) source=static address=192.168.1.50 mask=255.255.255.0 gateway=192.168.1.1"
        );
        assert_eq!(c[1].join(" "), "netsh interface ipv4 set dnsservers name=vEthernet (WSL) source=static address=1.1.1.1 register=none validate=no");
        assert_eq!(c[2].join(" "), "netsh interface ipv4 add dnsservers name=vEthernet (WSL) address=8.8.8.8 index=2 validate=no");
    }

    #[test]
    fn skips_unchanged_halves_and_rejects_bad_input() {
        let mut c = cfg();
        c.ip_auto = true;
        let only_ip = commands(&Request::Apply { name: "Wi-Fi".into(), cfg: c.clone(), ip: true, dns: false }).unwrap();
        assert_eq!(only_ip.len(), 1);
        assert!(only_ip[0].ends_with(&["source=dhcp".to_string()]));
        c.ip_auto = false;
        c.address = "300.1.1.1".into();
        assert!(commands(&Request::Apply { name: "Wi-Fi".into(), cfg: c, ip: true, dns: false }).is_err());
        assert!(commands(&Request::Renew { name: "a\" & calc".into() }).is_err());
    }
}
