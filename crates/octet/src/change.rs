//! Changes to Windows network settings. The UI asks for a `Request`; it is validated,
//! then handed on the command line to a second copy of Octet started with UAC
//! ("--elevated <hex json> <out file>"), which validates it again and runs
//! netsh/ipconfig directly (no shell, so adapter names cannot inject commands).
//! netsh targets the interface index, not the name, so a rename or a name like "2"
//! cannot hit the wrong adapter.

use crate::adapters::Cfg;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Request {
    /// `ip` / `dns`: which halves changed. netsh fails on "DHCP is already enabled",
    /// so an unchanged half is left alone.
    Apply { index: u32, name: String, cfg: Cfg, ip: bool, dns: bool },
    Release { index: u32, name: String },
    Renew { index: u32, name: String },
    Enable { index: u32, name: String, on: bool },
}

fn ipv4(v: &str, what: &str) -> Result<Ipv4Addr, String> {
    v.parse().map_err(|_| format!("{what} is not a valid IPv4 address"))
}

pub fn prefix_mask(prefix: u8) -> Ipv4Addr {
    Ipv4Addr::from(u32::MAX.checked_shl(32 - u32::from(prefix.min(32))).unwrap_or(0))
}

/// "255.255.255.0" -> 24. Rejects non-contiguous masks.
pub fn parse_subnet(value: &str) -> Result<u8, String> {
    let mask = u32::from(ipv4(value, "Subnet mask")?);
    let prefix = mask.leading_ones() as u8;
    if u32::from(prefix_mask(prefix)) == mask { Ok(prefix) } else { Err("Subnet mask bits must be contiguous".into()) }
}

/// The exact programs and arguments a request runs. Validates everything first.
pub fn commands(r: &Request) -> Result<Vec<Vec<String>>, String> {
    let (index, name) = match r {
        Request::Apply { index, name, .. }
        | Request::Release { index, name }
        | Request::Renew { index, name }
        | Request::Enable { index, name, .. } => (*index, name),
    };
    // ipconfig matches names as wildcards, so * and ? would hit every adapter.
    if name.is_empty() || name.chars().any(|c| c.is_control() || matches!(c, '"' | '*' | '?')) {
        return Err("Adapter name is not usable".into());
    }
    let cmd = |args: &[&str]| args.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let iface = format!("name={index}");
    Ok(match r {
        Request::Release { .. } => vec![cmd(&["ipconfig", "/release", name])],
        Request::Renew { .. } => vec![cmd(&["ipconfig", "/renew", name])],
        // `netsh interface set interface` only takes names, never an index.
        Request::Enable { on, .. } => {
            vec![cmd(&["netsh", "interface", "set", "interface", &format!("name={name}"), if *on { "admin=enable" } else { "admin=disable" }])]
        }
        Request::Apply { cfg, ip, dns, .. } => {
            if *ip && !cfg.ip_auto && cfg.dns_auto {
                return Err("A static IP needs manual DNS".into());
            }
            let mut out = vec![];
            if *ip {
                if cfg.ip_auto {
                    out.push(cmd(&["netsh", "interface", "ipv4", "set", "address", &iface, "source=dhcp"]));
                } else {
                    let address = ipv4(&cfg.address, "IP address")?;
                    let prefix = parse_subnet(&cfg.subnet)?;
                    if prefix == 0 {
                        return Err("Subnet mask cannot be 0.0.0.0".into());
                    }
                    let gateway = match cfg.gateway.as_str() {
                        "" => "gateway=none".to_string(),
                        g => format!("gateway={}", ipv4(g, "Gateway")?),
                    };
                    out.push(cmd(&[
                        "netsh", "interface", "ipv4", "set", "address", &iface, "source=static",
                        &format!("address={address}"), &format!("mask={}", prefix_mask(prefix)), &gateway,
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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

/// Console programs write in the OEM code page (e.g. 874 on Thai Windows), not UTF-8.
pub fn oem_text(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Globalization::{CP_OEMCP, MultiByteToWideChar};
        let n = MultiByteToWideChar(CP_OEMCP, 0, bytes.as_ptr(), bytes.len() as i32, std::ptr::null_mut(), 0);
        let mut w = vec![0u16; n.max(0) as usize];
        MultiByteToWideChar(CP_OEMCP, 0, bytes.as_ptr(), bytes.len() as i32, w.as_mut_ptr(), n);
        return String::from_utf16_lossy(&w);
    }
    #[allow(unreachable_code)]
    String::from_utf8_lossy(bytes).into_owned()
}

/// Unelevated side: validate, then start the elevated copy and wait for it.
pub fn elevate(r: &Request) -> Result<(), String> {
    let cmds = commands(r)?;
    if cmds.is_empty() {
        return Ok(());
    }
    // Already running as administrator (the "Always run as administrator" setting): no UAC.
    if crate::net::elevated() {
        return run(cmds);
    }
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let out = std::env::temp_dir().join(format!("octet-{}-{nanos}.out", std::process::id()));
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // The request travels on the command line, so nothing can swap it between the
    // UAC prompt and the elevated copy reading it. The .out file only carries error text.
    let params = format!("--elevated {} \"{}\"", hex(&serde_json::to_vec(r).map_err(|e| e.to_string())?), out.display());
    let code = run_as_admin(&exe, &params, true)?.unwrap_or(1);
    let detail = std::fs::read_to_string(&out).unwrap_or_default();
    let _ = std::fs::remove_file(&out);
    match code {
        0 => Ok(()),
        _ if detail.trim().is_empty() => Err(format!("The change failed (exit code {code})")),
        _ => Err(detail.trim().to_string()),
    }
}

/// Starts `exe` through UAC. With `wait`, returns its exit code once it ends.
#[cfg(windows)]
pub fn run_as_admin(exe: &std::path::Path, params: &str, wait: bool) -> Result<Option<u32>, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, GetLastError};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};
    use windows_sys::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
    let wide = |s: &std::ffi::OsStr| s.encode_wide().chain([0]).collect::<Vec<u16>>();
    let (verb, file, args) = (wide("runas".as_ref()), wide(exe.as_os_str()), wide(params.as_ref()));
    unsafe {
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = if wait { SEE_MASK_NOCLOSEPROCESS } else { 0 };
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = args.as_ptr();
        info.nShow = 0; // SW_HIDE
        if ShellExecuteExW(&mut info) == 0 {
            return Err(if GetLastError() == ERROR_CANCELLED {
                "Cancelled: administrator permission was not given".into()
            } else {
                format!("Could not start the elevated helper (error {})", GetLastError())
            });
        }
        if !wait {
            return Ok(None);
        }
        WaitForSingleObject(info.hProcess, INFINITE);
        let mut code = 1u32;
        GetExitCodeProcess(info.hProcess, &mut code);
        CloseHandle(info.hProcess);
        Ok(Some(code))
    }
}

#[cfg(not(windows))]
pub fn run_as_admin(_: &std::path::Path, _: &str, _: bool) -> Result<Option<u32>, String> {
    Err("Octet only changes settings on Windows".into())
}

/// Elevated side: run the request, write any failure to `out`.
pub fn run_elevated(request_hex: &str, out: &str) -> i32 {
    let fail = |msg: String| {
        let _ = std::fs::write(out, msg);
        1
    };
    let req: Request = match unhex(request_hex).and_then(|b| serde_json::from_slice(&b).ok()) {
        Some(r) => r,
        None => return fail("Could not read the change request".into()),
    };
    match commands(&req).and_then(run) {
        Ok(()) => 0,
        Err(e) => fail(e),
    }
}

/// Runs validated commands in order; stops at the first failure with its output.
fn run(cmds: Vec<Vec<String>>) -> Result<(), String> {
    for c in cmds {
        let o = crate::hidden(&c[0]).args(&c[1..]).output().map_err(|e| e.to_string())?;
        if !o.status.success() {
            let text = format!("{}{}", oem_text(&o.stdout), oem_text(&o.stderr));
            let text = text.trim();
            return Err(if text.is_empty() { format!("{} failed", c.join(" ")) } else { text.to_string() });
        }
    }
    Ok(())
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

    fn apply(cfg: Cfg, ip: bool, dns: bool) -> Request {
        Request::Apply { index: 12, name: "vEthernet (WSL)".into(), cfg, ip, dns }
    }

    #[test]
    fn builds_static_apply_by_index() {
        let c = commands(&apply(cfg(), true, true)).unwrap();
        assert_eq!(
            c[0].join(" "),
            "netsh interface ipv4 set address name=12 source=static address=192.168.1.50 mask=255.255.255.0 gateway=192.168.1.1"
        );
        assert_eq!(c[1].join(" "), "netsh interface ipv4 set dnsservers name=12 source=static address=1.1.1.1 register=none validate=no");
        assert_eq!(c[2].join(" "), "netsh interface ipv4 add dnsservers name=12 address=8.8.8.8 index=2 validate=no");
    }

    #[test]
    fn skips_unchanged_halves_and_rejects_bad_input() {
        let mut c = cfg();
        c.ip_auto = true;
        let only_ip = commands(&apply(c.clone(), true, false)).unwrap();
        assert_eq!(only_ip.len(), 1);
        assert!(only_ip[0].ends_with(&["source=dhcp".to_string()]));
        c.ip_auto = false;
        c.dns_auto = true;
        assert!(commands(&apply(c.clone(), true, true)).is_err(), "static IP with automatic DNS");
        c.dns_auto = false;
        c.address = "300.1.1.1".into();
        assert!(commands(&apply(c, true, false)).is_err());
        assert!(commands(&Request::Renew { index: 1, name: "a\" & calc".into() }).is_err());
        assert!(commands(&Request::Release { index: 1, name: "*".into() }).is_err());
    }

    #[test]
    fn request_survives_the_command_line() {
        let r = apply(cfg(), true, false);
        let back: Request = serde_json::from_slice(&unhex(&hex(&serde_json::to_vec(&r).unwrap())).unwrap()).unwrap();
        assert_eq!(commands(&back).unwrap(), commands(&r).unwrap());
        assert_eq!(parse_subnet("255.255.254.0"), Ok(23));
        assert!(parse_subnet("255.0.255.0").is_err());
    }
}
