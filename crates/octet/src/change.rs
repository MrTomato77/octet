//! Changes to network settings. The UI asks for a `Request`; it is validated,
//! then handed on the command line to a second copy of Octet started with UAC
//! ("--elevated <hex json> <out file>"), which validates it again and runs
//! netsh directly (no shell, so adapter names cannot inject commands).
//! netsh targets the interface index, not the name, so a rename or a name like "2"
//! cannot hit the wrong adapter.
//! On macOS the second copy is started as root through the administrator prompt and runs
//! networksetup, which only takes a service's name (see `networksetup`).

use crate::adapters::Cfg;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Request {
    /// `ip` / `dns`: which halves changed. netsh fails on "DHCP is already enabled",
    /// so an unchanged half is left alone. `extra`: the Advanced lists (more addresses,
    /// more gateways, metrics) changed; `was_extra` / `was_gws` are what the adapter has now,
    /// so only what was removed is deleted.
    Apply {
        index: u32,
        name: String,
        cfg: Cfg,
        ip: bool,
        dns: bool,
        #[serde(default)]
        extra: bool,
        #[serde(default, rename = "wasExtra")]
        was_extra: Vec<String>,
        #[serde(default, rename = "wasGws")]
        was_gws: Vec<String>,
    },
    Release { index: u32, name: String },
    Renew { index: u32, name: String },
    Enable { index: u32, name: String, on: bool },
    /// macOS only: emptying the DNS cache needs root there (Windows does it in-process).
    FlushDns,
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

/// Not a program: `run` does this step itself (DNS servers through the API).
const DNS: &str = "octet:set-dns";
/// Same: release or renew the DHCP lease through the API.
const LEASE: &str = "octet:lease";
/// macOS: wait until the new address is on the device (see `crate::mac::wait_address`).
#[cfg(target_os = "macos")]
const WAIT: &str = "octet:wait-address";

/// An extra address's mask: a dotted mask or a prefix length.
#[cfg(not(target_os = "macos"))]
fn mask_of(v: &str) -> Result<Ipv4Addr, String> {
    let prefix = match v.parse::<u8>() {
        Ok(p) if (1..=32).contains(&p) => p,
        Ok(_) => return Err("Extra IP prefix must be 1 to 32".into()),
        Err(_) => parse_subnet(v)?,
    };
    if prefix == 0 {
        return Err("Extra IP mask cannot be 0.0.0.0".into());
    }
    Ok(prefix_mask(prefix))
}

/// A metric: empty = automatic (None), else 1-9999.
#[cfg(not(target_os = "macos"))]
fn metric(v: &str) -> Result<Option<u16>, String> {
    match v.trim() {
        "" => Ok(None),
        m => m.parse::<u16>().ok().filter(|m| (1..=9999).contains(m)).map(Some).ok_or_else(|| "A metric is a number from 1 to 9999".into()),
    }
}

/// The exact programs and arguments a request runs. Validates everything first.
pub fn commands(r: &Request) -> Result<Vec<Vec<String>>, String> {
    let (index, name) = match r {
        Request::Apply { index, name, .. }
        | Request::Release { index, name }
        | Request::Renew { index, name }
        | Request::Enable { index, name, .. } => (*index, name),
        Request::FlushDns if cfg!(target_os = "macos") => return Ok(vec![cmd(&["dscacheutil", "-flushcache"]), cmd(&["killall", "-HUP", "mDNSResponder"])]),
        Request::FlushDns => return Ok(vec![]),
    };
    // * and ? are wildcards to some tools; a leading - would read as an option.
    if name.is_empty() || name.starts_with('-') || name.chars().any(|c| c.is_control() || matches!(c, '"' | '*' | '?')) {
        return Err("Adapter name is not usable".into());
    }
    platform(r, index, name)
}

/// Manual DNS servers in order; none = take them from DHCP. An Alternate typed in with Preferred
/// left empty still counts (it becomes the first): dropping it would quietly switch DNS to automatic.
fn dns_servers(cfg: &Cfg) -> Result<Vec<String>, String> {
    if cfg.dns_auto {
        return Ok(vec![]);
    }
    [(&cfg.dns1, "Preferred DNS"), (&cfg.dns2, "Alternate DNS")].into_iter().filter(|(v, _)| !v.is_empty())
        .map(|(v, what)| ipv4(v, what).map(|ip| ip.to_string())).collect()
}

fn cmd(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}

/// macOS: networksetup, by the service's name (it takes nothing else). Each service has one
/// address, so the Advanced lists don't apply; macOS has no lease release either.
#[cfg(target_os = "macos")]
fn platform(r: &Request, index: u32, name: &str) -> Result<Vec<Vec<String>>, String> {
    let ns = |args: &[&str]| [&["networksetup"], args].concat().iter().map(|s| s.to_string()).collect::<Vec<_>>();
    Ok(match r {
        Request::FlushDns => unreachable!(),
        Request::Release { .. } => return Err("macOS can't release a lease on its own; use Renew".into()),
        // In-process (SCNetworkInterfaceForceConfigurationRefresh), as System Settings' Renew DHCP Lease.
        Request::Renew { .. } => vec![cmd(&[LEASE, &index.to_string(), "renew"])],
        Request::Enable { on, .. } => vec![ns(&["-setnetworkserviceenabled", name, if *on { "on" } else { "off" }])],
        Request::Apply { cfg, ip, dns, extra, .. } => {
            if *ip && !cfg.ip_auto && cfg.dns_auto {
                return Err("A static IP needs manual DNS".into());
            }
            if *extra && (!cfg.extra.is_empty() || !cfg.gws.is_empty()) {
                return Err("macOS keeps one address per service; add a second service in System Settings for another".into());
            }
            let mut out = vec![];
            if *ip {
                if cfg.ip_auto {
                    out.push(ns(&["-setdhcp", name]));
                } else {
                    let address = ipv4(&cfg.address, "IP address")?.to_string();
                    let prefix = parse_subnet(&cfg.subnet)?;
                    if prefix == 0 {
                        return Err("Subnet mask cannot be 0.0.0.0".into());
                    }
                    let mask = prefix_mask(prefix).to_string();
                    let mut c = ns(&["-setmanual", name, &address, &mask]);
                    if !cfg.gateway.is_empty() {
                        c.push(ipv4(&cfg.gateway, "Gateway")?.to_string());
                    }
                    out.push(c);
                    out.push(cmd(&[WAIT, &index.to_string(), &address]));
                }
            }
            if *dns {
                let servers = dns_servers(cfg)?;
                // "Empty": the servers come from DHCP again.
                let mut c = ns(&["-setdnsservers", name]);
                c.extend(if servers.is_empty() { vec!["Empty".to_string()] } else { servers });
                out.push(c);
            }
            out
        }
    })
}

#[cfg(not(target_os = "macos"))]
fn platform(r: &Request, index: u32, name: &str) -> Result<Vec<Vec<String>>, String> {
    let iface = format!("name={index}");
    Ok(match r {
        Request::FlushDns => unreachable!(),
        Request::Release { .. } => vec![cmd(&[LEASE, &index.to_string(), "release"])],
        Request::Renew { .. } => vec![cmd(&[LEASE, &index.to_string(), "renew"])],
        // `netsh interface set interface` only takes names, never an index.
        Request::Enable { on, .. } => {
            vec![cmd(&["netsh", "interface", "set", "interface", &format!("name={name}"), if *on { "admin=enable" } else { "admin=disable" }])]
        }
        Request::Apply { cfg, ip, dns, extra, was_extra, was_gws, .. } => {
            if *ip && !cfg.ip_auto && cfg.dns_auto {
                return Err("A static IP needs manual DNS".into());
            }
            let mut out = vec![];
            // Every value is checked before any command is built.
            let extras = cfg.extra.iter().map(|a| Ok((ipv4(&a.address, "Extra IP address")?, mask_of(&a.subnet)?))).collect::<Result<Vec<_>, String>>()?;
            let gws = cfg.gws.iter().map(|g| Ok((ipv4(&g.gateway, "Extra gateway")?, metric(&g.metric)?))).collect::<Result<Vec<_>, String>>()?;
            let was_extra = was_extra.iter().map(|a| ipv4(a, "Current extra IP")).collect::<Result<Vec<_>, String>>()?;
            let was_gws = was_gws.iter().map(|g| ipv4(g, "Current extra gateway")).collect::<Result<Vec<_>, String>>()?;
            let (gw_metric, if_metric) = (metric(&cfg.gw_metric)?, metric(&cfg.metric)?);
            // `set address` (static or DHCP) clears every typed-in address and gateway, so after
            // it the Advanced lists go back on in full; otherwise only the difference is applied.
            let replaced = *ip;
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
                    let mut c = cmd(&[
                        "netsh", "interface", "ipv4", "set", "address", &iface, "source=static",
                        &format!("address={address}"), &format!("mask={}", prefix_mask(prefix)), &gateway,
                    ]);
                    if let (Some(m), false) = (gw_metric, cfg.gateway.is_empty()) {
                        c.push(format!("gwmetric={m}"));
                    }
                    out.push(c);
                }
            }
            if *extra || replaced {
                let keep = |a: &Ipv4Addr, list: &[Ipv4Addr]| replaced || !list.contains(a);
                for a in was_extra.iter().filter(|a| !replaced && !extras.iter().any(|(x, _)| x == *a)) {
                    out.push(cmd(&["netsh", "interface", "ipv4", "delete", "address", &iface, &format!("address={a}")]));
                }
                for g in was_gws.iter().filter(|g| !replaced && !gws.iter().any(|(x, _)| x == *g)) {
                    out.push(cmd(&["netsh", "interface", "ipv4", "delete", "address", &iface, &format!("gateway={g}")]));
                }
                let new_extra: Vec<_> = extras.iter().filter(|(a, _)| keep(a, &was_extra)).collect();
                // On DHCP, Windows keeps typed-in addresses beside the leased one only with coexistence on.
                if cfg.ip_auto && !new_extra.is_empty() {
                    out.push(cmd(&["netsh", "interface", "ipv4", "set", "interface", &format!("interface={index}"), "dhcpstaticipcoexistence=enabled"]));
                }
                for (a, m) in new_extra {
                    out.push(cmd(&["netsh", "interface", "ipv4", "add", "address", &iface, &format!("address={a}"), &format!("mask={m}")]));
                }
                for (g, m) in gws.iter().filter(|(g, _)| keep(g, &was_gws)) {
                    let mut c = cmd(&["netsh", "interface", "ipv4", "add", "address", &iface, &format!("gateway={g}")]);
                    c.push(format!("gwmetric={}", m.unwrap_or(0)));   // 0 = automatic
                    out.push(c);
                }
                if let Some(m) = if_metric {
                    out.push(cmd(&["netsh", "interface", "ipv4", "set", "interface", &format!("interface={index}"), &format!("metric={m}")]));
                }
            }
            if *dns {
                // Done in-process with SetInterfaceDnsSettings (what Windows Settings uses): no netsh,
                // well under 10 ms. No servers = take them from DHCP.
                let servers = dns_servers(cfg)?;
                out.push(cmd(&[DNS, &index.to_string(), &servers.join(","), if cfg.register { "register" } else { "noregister" }]));
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
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    return crate::mac::sudo(&exe, &hex(&serde_json::to_vec(r).map_err(|e| e.to_string())?));
    #[allow(unreachable_code)]
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let out = std::env::temp_dir().join(format!("octet-{}-{nanos}.out", std::process::id()));
    // Made here, by this (unelevated) copy: the elevated one only writes into it (`write_out`).
    let made = std::fs::OpenOptions::new().write(true).create_new(true).open(&out).map_err(|e| format!("Could not prepare the change: {e}"))?;
    let id = file_id(&made).ok_or("Could not prepare the change")?;
    drop(made);
    // The request travels on the command line, so nothing can swap it between the
    // UAC prompt and the elevated copy reading it. The .out file only carries error text.
    let params = format!("--elevated {} \"{}\" {id}", hex(&serde_json::to_vec(r).map_err(|e| e.to_string())?), out.display());
    let code = run_as_admin(&exe, &params, true);
    let detail = std::fs::read_to_string(&out).unwrap_or_default();
    let _ = std::fs::remove_file(&out);   // also when UAC was declined
    let code = code?.unwrap_or(1);
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

/// macOS asks for the password with each change instead (see `crate::mac::sudo`): a whole
/// Octet running as root would put a root process on the user's screen.
#[cfg(not(windows))]
pub fn run_as_admin(_: &std::path::Path, _: &str, _: bool) -> Result<Option<u32>, String> {
    Err("On macOS, Octet asks for your password with each change".into())
}

/// Which file this is, wherever its path leads: volume and file index ("" off Windows).
#[cfg(windows)]
fn file_id(f: &std::fs::File) -> Option<String> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle};
    let mut i: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    (unsafe { GetFileInformationByHandle(f.as_raw_handle(), &mut i) } != 0)
        .then(|| format!("{:x}-{:x}-{:x}", i.dwVolumeSerialNumber, i.nFileIndexHigh, i.nFileIndexLow))
}

#[cfg(not(windows))]
fn file_id(_: &std::fs::File) -> Option<String> {
    Some(String::new())
}

/// Elevated side: run the request, write any failure to `out`. Out "-" (macOS): print it and
/// exit 0, so the administrator prompt's caller gets the text as the command's output.
/// The elevated copy writes its error text only into the file the unelevated side made for it: never creating
/// one, never through a link, and only into that very file (`id`), so a folder on the way swapped for a junction
/// can't aim an admin write at another file either.
fn write_out(out: &str, id: Option<&str>, msg: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut o = std::fs::OpenOptions::new();
    o.write(true);
    #[cfg(windows)]
    std::os::windows::fs::OpenOptionsExt::custom_flags(&mut o, 0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT
    let mut f = o.open(out)?;
    if !f.metadata()?.file_type().is_file() || file_id(&f).as_deref() != id {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    f.set_len(0)?;   // emptied only once it's known to be the right file
    f.write_all(msg.as_bytes())
}

pub fn run_elevated(request_hex: &str, out: &str, id: Option<&str>) -> i32 {
    let fail = |msg: String| {
        if out == "-" {
            print!("{msg}");
            return 0;
        }
        let _ = write_out(out, id, &msg);
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
        if c[0] == DNS {
            let index = c[1].parse().map_err(|_| "Bad interface index".to_string())?;
            crate::net::set_dns(index, &c[2], c[3] == "register")?;
            continue;
        }
        #[cfg(target_os = "macos")]
        if c[0] == WAIT {
            let (index, address) = (c[1].parse().map_err(|_| "Bad interface index".to_string())?, ipv4(&c[2], "IP address")?);
            crate::mac::wait_address(index, address);
            continue;
        }
        if c[0] == LEASE {
            let index = c[1].parse().map_err(|_| "Bad interface index".to_string())?;
            crate::net::dhcp(index, c[2] == "renew")?;
            continue;
        }
        let o = crate::hidden(&c[0]).args(&c[1..]).output().map_err(|e| e.to_string())?;
        // networksetup can say "** Error: ..." and still exit 0.
        if !o.status.success() || (c[0] == "networksetup" && String::from_utf8_lossy(&o.stdout).contains("** Error")) {
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
            ..Default::default()
        }
    }

    fn apply(cfg: Cfg, ip: bool, dns: bool) -> Request {
        Request::Apply { index: 12, name: "vEthernet (WSL)".into(), cfg, ip, dns, extra: false, was_extra: vec![], was_gws: vec![] }
    }

    #[test]
    fn writes_errors_only_into_a_file_made_for_them() {
        let p = std::env::temp_dir().join(format!("octet-test-{}.out", std::process::id()));
        let _ = std::fs::remove_file(&p);
        let path = p.to_str().unwrap();
        assert!(super::write_out(path, None, "x").is_err() && !p.exists());
        std::fs::write(&p, "old text").unwrap();
        let id = super::file_id(&std::fs::File::open(&p).unwrap());
        super::write_out(path, id.as_deref(), "new").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "new");
        // Another file at that path (a folder on the way swapped for a junction, say) is not the one made for it.
        std::fs::remove_file(&p).unwrap();
        std::fs::write(&p, "someone else's").unwrap();
        assert!(super::write_out(path, id.as_deref(), "x").is_err());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "someone else's");
        std::fs::remove_file(&p).unwrap();
        assert!(super::write_out(std::env::temp_dir().to_str().unwrap(), id.as_deref(), "x").is_err());
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn extra_addresses_beside_dhcp_and_static() {
        use crate::adapters::{Addr, Gw};
        let mut c = cfg();
        c.ip_auto = true;
        c.extra = vec![Addr { address: "192.168.1.60".into(), subnet: "24".into() }, Addr { address: "10.9.0.1".into(), subnet: "255.255.0.0".into() }];
        // Only the Advanced list changed, on DHCP: coexistence, then the new one; the kept one and the main address untouched.
        let r = Request::Apply { index: 12, name: "Wi-Fi".into(), cfg: c.clone(), ip: false, dns: false, extra: true,
            was_extra: vec!["192.168.1.60".into(), "172.16.0.9".into()], was_gws: vec![] };
        let got: Vec<String> = commands(&r).unwrap().iter().map(|c| c.join(" ")).collect();
        // The elevated copy gets the request as JSON and must build the same commands.
        let sent: Request = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
        assert_eq!(commands(&sent).unwrap(), commands(&r).unwrap(), "the Advanced lists survive the trip to the elevated copy");
        assert_eq!(got, [
            "netsh interface ipv4 delete address name=12 address=172.16.0.9",
            "netsh interface ipv4 set interface interface=12 dhcpstaticipcoexistence=enabled",
            "netsh interface ipv4 add address name=12 address=10.9.0.1 mask=255.255.0.0",
        ]);
        // Static: set address replaces all, so every extra address and gateway goes back on.
        let mut s = cfg();
        s.extra = vec![Addr { address: "192.168.1.60".into(), subnet: "255.255.255.0".into() }];
        s.gws = vec![Gw { gateway: "192.168.1.2".into(), metric: "20".into() }];
        s.gw_metric = "10".into();
        let r = Request::Apply { index: 12, name: "Wi-Fi".into(), cfg: s, ip: true, dns: false, extra: true, was_extra: vec!["192.168.1.60".into()], was_gws: vec![] };
        let got: Vec<String> = commands(&r).unwrap().iter().map(|c| c.join(" ")).collect();
        assert_eq!(got, [
            "netsh interface ipv4 set address name=12 source=static address=192.168.1.50 mask=255.255.255.0 gateway=192.168.1.1 gwmetric=10",
            "netsh interface ipv4 add address name=12 address=192.168.1.60 mask=255.255.255.0",
            "netsh interface ipv4 add address name=12 gateway=192.168.1.2 gwmetric=20",
        ]);
        // Bad Advanced values stop everything before any command.
        c.extra[0].subnet = "33".into();
        assert!(commands(&Request::Apply { index: 1, name: "a".into(), cfg: c, ip: false, dns: false, extra: true, was_extra: vec![], was_gws: vec![] }).is_err());
        let mut m = cfg();
        m.metric = "0".into();
        assert!(commands(&Request::Apply { index: 1, name: "a".into(), cfg: m, ip: false, dns: false, extra: true, was_extra: vec![], was_gws: vec![] }).is_err());
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn builds_static_apply_by_index() {
        let c = commands(&apply(cfg(), true, true)).unwrap();
        assert_eq!(
            c[0].join(" "),
            "netsh interface ipv4 set address name=12 source=static address=192.168.1.50 mask=255.255.255.0 gateway=192.168.1.1"
        );
        assert_eq!(c[1].join(" "), "octet:set-dns 12 1.1.1.1,8.8.8.8 noregister");
        assert_eq!(c.len(), 2);
        let mut auto = cfg();
        (auto.ip_auto, auto.dns_auto, auto.register) = (true, true, true);
        assert_eq!(commands(&apply(auto, false, true)).unwrap()[0].join(" "), "octet:set-dns 12  register", "automatic DNS = no servers");
        let mut alt = cfg();
        alt.dns1 = String::new();
        assert_eq!(commands(&apply(alt, false, true)).unwrap()[0].join(" "), "octet:set-dns 12 8.8.8.8 noregister", "an Alternate alone is kept, not dropped");
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
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
    #[cfg(target_os = "macos")]
    fn builds_networksetup_by_service_name() {
        let got = |r: &Request| commands(r).map(|c| c.iter().map(|c| c.join(" ")).collect::<Vec<_>>());
        let wifi = |cfg: Cfg, ip, dns| Request::Apply { index: 12, name: "Wi-Fi".into(), cfg, ip, dns, extra: false, was_extra: vec![], was_gws: vec![] };
        assert_eq!(got(&wifi(cfg(), true, true)).unwrap(), [
            "networksetup -setmanual Wi-Fi 192.168.1.50 255.255.255.0 192.168.1.1",
            "octet:wait-address 12 192.168.1.50",
            "networksetup -setdnsservers Wi-Fi 1.1.1.1 8.8.8.8",
        ]);
        let mut c = cfg();
        (c.ip_auto, c.dns_auto, c.gateway) = (true, true, String::new());
        assert_eq!(got(&wifi(c.clone(), true, true)).unwrap(), ["networksetup -setdhcp Wi-Fi", "networksetup -setdnsservers Wi-Fi Empty"]);
        c.ip_auto = false;
        assert!(commands(&wifi(c.clone(), true, false)).is_err(), "static IP with automatic DNS");
        c.dns_auto = false;
        assert_eq!(got(&wifi(c, true, false)).unwrap(), ["networksetup -setmanual Wi-Fi 192.168.1.50 255.255.255.0", "octet:wait-address 12 192.168.1.50"], "no gateway, no router");
        assert_eq!(got(&Request::Enable { index: 4, name: "USB LAN".into(), on: false }).unwrap(), ["networksetup -setnetworkserviceenabled USB LAN off"]);
        // The name is one argument however it is spelled; one that reads as an option is refused.
        assert_eq!(commands(&Request::Enable { index: 4, name: "a b; rm".into(), on: true }).unwrap()[0][2], "a b; rm");
        assert!(commands(&Request::Enable { index: 4, name: "-listallnetworkservices".into(), on: true }).is_err());
        assert!(commands(&Request::Release { index: 4, name: "Wi-Fi".into() }).is_err());
        assert_eq!(got(&Request::Renew { index: 4, name: "Wi-Fi".into() }).unwrap(), ["octet:lease 4 renew"]);
        assert_eq!(got(&Request::FlushDns).unwrap(), ["dscacheutil -flushcache", "killall -HUP mDNSResponder"]);
        let mut x = cfg();
        x.extra = vec![crate::adapters::Addr { address: "10.0.0.2".into(), subnet: "24".into() }];
        assert!(commands(&Request::Apply { index: 1, name: "Wi-Fi".into(), cfg: x, ip: false, dns: false, extra: true, was_extra: vec![], was_gws: vec![] }).is_err());
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
