//! Changes to network settings. The UI asks for a `Request`; it is validated,
//! then handed on the command line to a second copy of Octet started with UAC
//! ("--elevated <hex json> <out file>"), which validates it again and runs
//! the change in-process through Windows' own APIs (see `crate::win`); netsh is left only for DHCP/static
//! coexistence, which no documented API was found for.
//! netsh targets the interface index, not the name, so a rename or a name like "2"
//! cannot hit the wrong adapter.
//! On macOS the second copy is started as root through the administrator prompt and runs
//! networksetup, which only takes a service's name (see `networksetup`).

use crate::adapters::{Addr, Cfg, Gw};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Request {
    /// `ip` / `dns`: which halves changed. netsh fails on "DHCP is already enabled",
    /// so an unchanged half is left alone. `extra`: the Advanced lists (more addresses,
    /// more gateways, metrics) changed; `was_extra` / `was_gws` / `was_metric` are what the adapter has now,
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
        was_extra: Vec<Addr>,
        #[serde(default, rename = "wasGws")]
        was_gws: Vec<Gw>,
        #[serde(default, rename = "wasMetric")]
        was_metric: String,
        #[serde(default, rename = "wasFirst")]
        was_first: String,
    },
    Release { index: u32, name: String },
    /// `cycle`: release first, in the same elevated run (one password prompt on macOS).
    Renew { index: u32, name: String, #[serde(default)] cycle: bool },
    Enable { index: u32, name: String, on: bool },
    /// macOS only: emptying the DNS cache needs root there (Windows does it in-process).
    FlushDns,
    /// Internet Sharing from `public` (has the internet) to `private`: Windows' ICS, or macOS's Internet Sharing.
    /// `restore` (turning off only): the private adapter's saved settings, put back in the same elevated run,
    /// so Stop asks for admin once.
    Share { public: u32, private: u32, on: bool, #[serde(default)] scope: Option<Ipv4Addr>, #[serde(default)] restore: Option<Box<Request>> },
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
#[cfg(windows)]
const ADDRESS: &str = "octet:address";
#[cfg(windows)]
const ENABLE: &str = "octet:enable";
#[cfg(windows)]
const METRIC: &str = "octet:metric";
const SHARE: &str = "octet:share";
/// Windows: one more or one fewer Advanced address / default gateway (see `crate::win`).
#[cfg(windows)]
const XADDR: &str = "octet:extra-address";
#[cfg(windows)]
const GW: &str = "octet:gateway";
/// Windows: the gateway ahead of DHCP's (see `crate::win::first`); macOS goes by service order instead.
#[cfg(not(target_os = "macos"))]
const FIRST: &str = "octet:first";
/// macOS: wait until the new address is on the device (see `crate::mac::wait_address`).
#[cfg(target_os = "macos")]
const WAIT: &str = "octet:wait-address";

/// An extra address's mask: a dotted mask or a prefix length.
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
fn metric(v: &str) -> Result<Option<u32>, String> {
    match v.trim() {
        "" => Ok(None),
        // Typed ones are 1-9999 (the UI checks); larger ones come from Windows itself (a VPN's routes) and pass back as they are.
        m => m.parse::<u32>().ok().filter(|m| *m >= 1).map(Some).ok_or_else(|| "A metric is a number from 1 to 9999".into()),
    }
}

/// The exact programs and arguments a request runs. Validates everything first.
pub fn commands(r: &Request) -> Result<Vec<Vec<String>>, String> {
    let (index, name) = match r {
        Request::Apply { index, name, .. }
        | Request::Release { index, name }
        | Request::Renew { index, name, .. }
        | Request::Enable { index, name, .. } => (*index, name),
        Request::FlushDns if cfg!(target_os = "macos") => return Ok(vec![cmd(&["dscacheutil", "-flushcache"]), cmd(&["killall", "-HUP", "mDNSResponder"])]),
        Request::FlushDns => return Ok(vec![]),
        Request::Share { public, private, on, scope, restore } => {
            let mut c = vec![SHARE.to_string(), public.to_string(), private.to_string(), (if *on { "on" } else { "off" }).into()];
            if let Some(ip) = scope.filter(|_| *on) {
                let o = ip.octets();
                if !ip.is_private() || o[3] == 0 || o[3] == 255 { return Err(format!("{ip} is not a private host address for sharing")); }
                // macOS gives this Mac the network's .1 and hands out the rest.
                if cfg!(target_os = "macos") && o[3] != 1 { return Err(format!("On a Mac, sharing takes the .1 address of the network: {}.{}.{}.1", o[0], o[1], o[2])); }
                c.push(ip.to_string());
            }
            let mut all = vec![c];
            match restore.as_deref() {
                None => {}
                Some(r @ Request::Apply { index, .. }) if !*on && index == private => all.extend(commands(r)?),
                Some(_) => return Err("Only the shared adapter's own settings can be put back after sharing".into()),
            }
            return Ok(all);
        }
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

/// Whether `ip` is on the network `net`/`prefix`.
#[cfg(target_os = "macos")]
fn in_net(ip: Ipv4Addr, net: Ipv4Addr, prefix: u8) -> bool {
    let m = u32::from(prefix_mask(prefix));
    u32::from(ip) & m == u32::from(net) & m
}

/// macOS: networksetup, by the service's name (it takes nothing else).
/// Release turns the service's IPv4 off (macOS has no release of its own; System Settings' Configure IPv4: Off
/// does the same) and Renew turns DHCP back on. Each service has one address and one router, so an Advanced
/// address is a service of its own on the same port (`crate::mac::extra_name`), and each gateway goes with the
/// first address whose network holds it, as the Advanced page draws it.
#[cfg(target_os = "macos")]
fn platform(r: &Request, index: u32, name: &str) -> Result<Vec<Vec<String>>, String> {
    let ns = |args: &[&str]| [&["networksetup"], args].concat().iter().map(|s| s.to_string()).collect::<Vec<_>>();
    Ok(match r {
        Request::FlushDns | Request::Share { .. } => unreachable!(),
        Request::Release { .. } => vec![ns(&["-setv4off", name])],
        // In-process: DHCP back on after a Release, else SCNetworkInterfaceForceConfigurationRefresh (System Settings' Renew DHCP Lease).
        Request::Renew { cycle, .. } => {
            let mut out = if *cycle { vec![ns(&["-setv4off", name])] } else { vec![] };
            out.push(cmd(&[LEASE, &index.to_string(), "renew"]));
            out
        }
        Request::Enable { on, .. } => vec![ns(&["-setnetworkserviceenabled", name, if *on { "on" } else { "off" }])],
        Request::Apply { cfg, ip, dns, extra, was_extra, was_metric, was_first, .. } => {
            if *ip && !cfg.ip_auto && cfg.dns_auto {
                return Err("A static IP needs manual DNS".into());
            }
            if cfg.first.trim() != was_first.trim() {
                return Err("On a Mac a gateway can't go ahead of DHCP's; the service order decides (System Settings > Network > ... > Set Service Order)".into());
            }
            if cfg.metric.trim() != was_metric.trim() || !cfg.gw_metric.trim().is_empty() || cfg.gws.iter().any(|g| !g.metric.trim().is_empty()) {
                return Err("macOS has no metrics; it goes by service order (System Settings > Network > ... > Set Service Order)".into());
            }
            // Every value is checked before any command is built.
            let main: Option<(Ipv4Addr, u8)> = match (cfg.address.parse::<Ipv4Addr>(), parse_subnet(&cfg.subnet)) {
                (Ok(a), Ok(p)) if p > 0 => Some((a, p)),
                _ => None,
            };
            let extras = cfg.extra.iter().map(|a| Ok((ipv4(&a.address, "Extra IP address")?, mask_of(&a.subnet)?))).collect::<Result<Vec<_>, String>>()?;
            let was: Vec<Ipv4Addr> = was_extra.iter().filter_map(|a| a.address.parse().ok()).collect();
            let mut seen: Vec<Ipv4Addr> = main.map(|m| m.0).into_iter().collect();
            for (a, _) in &extras {
                if seen.contains(a) { return Err(format!("{a} is listed twice")); }
                seen.push(*a);
            }
            // Each gateway onto the first address whose network holds it. On DHCP the first address's router is the lease's.
            let mut gws: Vec<Ipv4Addr> = vec![];
            for g in std::iter::once(&cfg.gateway).chain(cfg.gws.iter().map(|g| &g.gateway)).filter(|g| !g.is_empty()) {
                let g = ipv4(g, "Gateway")?;
                if !gws.contains(&g) { gws.push(g); }
            }
            let (mut main_router, mut routers) = (None, vec![None; extras.len()]);
            for g in gws {
                if let Some((a, p)) = main && in_net(g, a, p) {
                    if cfg.ip_auto { continue; }
                    if main_router.is_some() { return Err(format!("Gateway {g}: on a Mac each address has one gateway")); }
                    main_router = Some(g);
                } else if let Some(i) = extras.iter().position(|&(a, m)| in_net(g, a, u32::from(m).leading_ones() as u8)) {
                    if routers[i].is_some() { return Err(format!("Gateway {g}: on a Mac each address has one gateway")); }
                    routers[i] = Some(g);
                } else if !cfg.ip_auto && main_router.is_none() && cfg.gateway.parse() == Ok(g) {
                    main_router = Some(g);   // a gateway off the address's network, as it could be set before
                } else {
                    return Err(format!("Gateway {g} isn't on the network of any address here; on a Mac a gateway goes with an address"));
                }
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
                    c.extend(main_router.map(|g| g.to_string()));
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
            if *extra || *ip {
                for a in was.iter().filter(|a| !extras.iter().any(|(e, _)| e == *a)) {
                    out.push(ns(&["-removenetworkservice", &crate::mac::extra_name(name, &a.to_string())]));
                }
                for ((a, m), router) in extras.iter().zip(&routers) {
                    let svc = crate::mac::extra_name(name, &a.to_string());
                    // A new one starts as a copy of this service (same port), then gets its own address and nothing else:
                    // no DNS servers of its own, no IPv6 (the first service has those).
                    let new = !was.contains(a);
                    if new {
                        out.push(ns(&["-duplicatenetworkservice", name, &svc]));
                    }
                    let mut c = ns(&["-setmanual", &svc, &a.to_string(), &m.to_string()]);
                    c.extend(router.map(|g| g.to_string()));
                    out.push(c);
                    if new {
                        out.push(ns(&["-setdnsservers", &svc, "Empty"]));
                        out.push(ns(&["-setv6off", &svc]));
                    }
                }
            }
            out
        }
    })
}

#[cfg(not(target_os = "macos"))]
fn platform(r: &Request, index: u32, _name: &str) -> Result<Vec<Vec<String>>, String> {
    Ok(match r {
        Request::FlushDns | Request::Share { .. } => unreachable!(),
        Request::Release { .. } => vec![cmd(&[LEASE, &index.to_string(), "release"])],
        Request::Renew { cycle, .. } => {
            let mut out = if *cycle { vec![cmd(&[LEASE, &index.to_string(), "release"])] } else { vec![] };
            out.push(cmd(&[LEASE, &index.to_string(), "renew"]));
            out
        }
        Request::Enable { on, .. } => {
            vec![cmd(&[ENABLE, &index.to_string(), if *on { "on" } else { "off" }])]
        }
        Request::Apply { cfg, ip, dns, extra, was_extra, was_gws, was_metric, was_first, .. } => {
            if *ip && !cfg.ip_auto && cfg.dns_auto {
                return Err("A static IP needs manual DNS".into());
            }
            let mut out = vec![];
            // Every value is checked before any command is built.
            let extras = cfg.extra.iter().map(|a| Ok((ipv4(&a.address, "Extra IP address")?, mask_of(&a.subnet)?))).collect::<Result<Vec<_>, String>>()?;
            let gws = cfg.gws.iter().map(|g| Ok((ipv4(&g.gateway, "Extra gateway")?, metric(&g.metric)?))).collect::<Result<Vec<_>, String>>()?;
            // What the adapter has now, whole: an entry whose mask or metric changed is taken off and put back.
            let was_extra = was_extra.iter().map(|a| Ok((ipv4(&a.address, "Current extra IP")?, mask_of(&a.subnet)?))).collect::<Result<Vec<_>, String>>()?;
            let was_gws = was_gws.iter().map(|g| Ok((ipv4(&g.gateway, "Current extra gateway")?, metric(&g.metric).unwrap_or(None)))).collect::<Result<Vec<_>, String>>()?;
            // netsh answers a repeat with "object already exists" halfway through the run: caught here, before anything.
            let mut seen: Vec<Ipv4Addr> = cfg.address.parse().ok().filter(|_| !cfg.ip_auto).into_iter().collect();
            for (a, _) in &extras {
                if seen.contains(a) { return Err(format!("{a} is listed twice")); }
                seen.push(*a);
            }
            let mut seen: Vec<Ipv4Addr> = cfg.gateway.parse().ok().filter(|_| !cfg.ip_auto).into_iter().collect();
            for (g, _) in &gws {
                if seen.contains(g) { return Err(format!("Gateway {g} is listed twice")); }
                seen.push(*g);
            }
            let gw_metric = metric(&cfg.gw_metric)?;
            // The interface metric is set only when it changed; emptied, it goes back to automatic.
            let if_metric = if cfg.metric.trim() != was_metric.trim() { Some(metric(&cfg.metric)?) } else { None };
            // `set address` (static or DHCP) clears every typed-in address and gateway, so after
            // it the Advanced lists go back on in full; otherwise only the difference is applied.
            let replaced = *ip;
            if *ip {
                if cfg.ip_auto {
                    out.push(cmd(&[ADDRESS, &index.to_string(), "dhcp"]));
                } else {
                    let address = ipv4(&cfg.address, "IP address")?;
                    let prefix = parse_subnet(&cfg.subnet)?;
                    if prefix == 0 {
                        return Err("Subnet mask cannot be 0.0.0.0".into());
                    }
                    let gateway = if cfg.gateway.is_empty() { None } else { Some(ipv4(&cfg.gateway, "Gateway")?) };
                    out.push(cmd(&[ADDRESS, &index.to_string(), &address.to_string(), &prefix_mask(prefix).to_string()]));
                    if let Some(gateway) = gateway {
                        out.push(cmd(&[GW, &index.to_string(), "add", &gateway.to_string(), &gw_metric.unwrap_or(0).to_string()]));
                    }
                }
            }
            // DNS goes on right after the address: if a later Advanced step fails, the adapter still has its DNS
            // (a switch to static drops the DHCP-given servers, and a static IP without DNS reaches nothing by name).
            if *dns {
                // Done in-process with SetInterfaceDnsSettings (what Windows Settings uses): no netsh,
                // well under 10 ms. No servers = take them from DHCP.
                let servers = dns_servers(cfg)?;
                out.push(cmd(&[DNS, &index.to_string(), &servers.join(","), if cfg.register { "register" } else { "noregister" }]));
            }
            if *extra || replaced {
                // Back to DHCP: the router hands out the gateway, and typed-in gateways from before (likely from the
                // old subnet) don't go back on. New ones typed in the Advanced list still do.
                let gws: Vec<_> = gws.iter().filter(|g| !(replaced && cfg.ip_auto && was_gws.contains(g))).collect();
                for a in was_extra.iter().filter(|a| !replaced && !extras.contains(a)) {
                    out.push(cmd(&[XADDR, &index.to_string(), "del", &a.0.to_string()]));
                }
                for g in was_gws.iter().filter(|g| !replaced && !gws.contains(g)) {
                    out.push(cmd(&[GW, &index.to_string(), "del", &g.0.to_string()]));
                }
                // The last typed-in address gone from a DHCP adapter: coexistence goes off again.
                if cfg.ip_auto && extras.is_empty() && !was_extra.is_empty() {
                    out.push(cmd(&["netsh", "interface", "ipv4", "set", "interface", &format!("interface={index}"), "dhcpstaticipcoexistence=disabled"]));
                }
                let new_extra: Vec<_> = extras.iter().filter(|a| replaced || !was_extra.contains(a)).collect();
                // On DHCP, Windows keeps typed-in addresses beside the leased one only with coexistence on.
                if cfg.ip_auto && !new_extra.is_empty() {
                    out.push(cmd(&["netsh", "interface", "ipv4", "set", "interface", &format!("interface={index}"), "dhcpstaticipcoexistence=enabled"]));
                }
                for (a, m) in new_extra {
                    out.push(cmd(&[XADDR, &index.to_string(), "add", &a.to_string(), &u32::from(*m).leading_ones().to_string()]));
                }
                for (g, m) in gws.into_iter().filter(|g| replaced || !was_gws.contains(g)) {
                    out.push(cmd(&[GW, &index.to_string(), "add", &g.to_string(), &m.unwrap_or(0).to_string()]));   // 0 = automatic
                }
                if let Some(m) = if_metric {
                    out.push(vec![METRIC.into(), index.to_string(), m.map(|v| v.to_string()).unwrap_or_else(|| "auto".into())]);
                }
                // Ahead of DHCP: the two /1 halves through the chosen gateway, or none.
                if cfg.first.trim() != was_first.trim() {
                    let first = if cfg.first.trim().is_empty() { "none".to_string() } else { ipv4(&cfg.first, "First gateway")?.to_string() };
                    out.push(cmd(&[FIRST, &index.to_string(), &first]));
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
    {
        use std::os::windows::fs::OpenOptionsExt;
        o.share_mode(0).custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT; held alone: nothing writes between check and write
    }
    let mut f = o.open(out)?;
    // Only into an empty file (the one made for it is): a forged launch that names some other file by its id
    // can't cut what's in it.
    let m = f.metadata()?;
    if !m.file_type().is_file() || m.len() != 0 || file_id(&f).as_deref() != id {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
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
        #[cfg(windows)]
        if matches!(c[0].as_str(), ADDRESS | XADDR | GW | FIRST | ENABLE | METRIC | SHARE) {
            let index = c[1].parse().map_err(|_| "Bad interface index")?;
            match c[0].as_str() {
                ADDRESS => crate::win::set_address(index, c.get(3).map(|m| (c[2].as_str(), m.as_str())))?,
                XADDR => crate::win::extra_address(index, &c[3], c.get(4).map(|p| p.parse().map_err(|_| "Bad prefix")).transpose()?)?,
                GW => crate::win::gateway(index, &c[3], c.get(4).map(|m| m.parse().map_err(|_| "Bad metric")).transpose()?)?,
                FIRST => crate::win::first(index, Some(c[2].as_str()).filter(|g| *g != "none"))?,
                ENABLE => crate::win::enable(index, c[2] == "on")?,
                METRIC => crate::win::set_metric(index, if c[2] == "auto" { None } else { Some(c[2].parse().map_err(|_| "Bad metric")?) })?,
                SHARE => crate::ics::set(index, c[2].parse().map_err(|_| "Bad interface index")?, c[3] == "on", c.get(4).map(String::as_str))?,
                _ => unreachable!(),
            }
            continue;
        }
        #[cfg(target_os = "macos")]
        if c[0] == SHARE {
            let (public, private) = (c[1].parse().map_err(|_| "Bad interface index")?, c[2].parse().map_err(|_| "Bad interface index")?);
            crate::mac::share_set(public, private, c[3] == "on", c.get(4).map(|ip| ipv4(ip, "Sharing address")).transpose()?)?;
            continue;
        }
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

    #[test]
    #[cfg(windows)]
    fn enable_uses_native_index_even_after_rename() {
        let commands = commands(&Request::Enable { index: 12, name: "renamed adapter".into(), on: false }).unwrap();
        assert_eq!(commands, [cmd(&[ENABLE, "12", "off"])]);
    }

    #[test]
    #[cfg(windows)]
    fn share_request_from_the_ui() {
        // Stop puts the shared adapter back in the same run; anything else as `restore` is refused.
        let back = r#"{"op":"apply","index":11,"name":"Ethernet 4","cfg":{"ipAuto":true,"address":"","subnet":"","gateway":"","dnsAuto":true,"dns1":"","dns2":"","register":true},"ip":true,"dns":true}"#;
        let stop: Request = serde_json::from_str(&format!(r#"{{"op":"share","public":18,"private":11,"on":false,"restore":{back}}}"#)).unwrap();
        let c = commands(&stop).unwrap();
        assert!(c.len() > 1 && c[0][0] == SHARE && c[0][3] == "off", "{c:?}");
        let other: Request = serde_json::from_str(&format!(r#"{{"op":"share","public":18,"private":12,"on":false,"restore":{back}}}"#)).unwrap();
        assert!(commands(&other).is_err(), "another adapter's settings");
        let start: Request = serde_json::from_str(&format!(r#"{{"op":"share","public":18,"private":11,"on":true,"restore":{back}}}"#)).unwrap();
        assert!(commands(&start).is_err(), "restore only when turning off");
        let r: Request = serde_json::from_str(r#"{"op":"share","public":18,"private":11,"on":true}"#).unwrap();
        assert_eq!(commands(&r).unwrap(), [cmd(&[SHARE, "18", "11", "on"])]);
        let r: Request = serde_json::from_str(r#"{"op":"share","public":18,"private":11,"on":true,"scope":"10.9.8.1"}"#).unwrap();
        assert_eq!(commands(&r).unwrap(), [cmd(&[SHARE, "18", "11", "on", "10.9.8.1"])]);
        let r: Request = serde_json::from_str(r#"{"op":"share","public":18,"private":11,"on":true,"scope":"8.8.8.8"}"#).unwrap();
        assert!(commands(&r).is_err(), "public address");
    }

    fn apply(cfg: Cfg, ip: bool, dns: bool) -> Request {
        Request::Apply { index: 12, name: "vEthernet (WSL)".into(), cfg, ip, dns, extra: false, was_extra: vec![], was_gws: vec![], was_metric: String::new(), was_first: String::new() }
    }

    #[test]
    fn writes_errors_only_into_a_file_made_for_them() {
        let p = std::env::temp_dir().join(format!("octet-test-{}.out", std::process::id()));
        let _ = std::fs::remove_file(&p);
        let path = p.to_str().unwrap();
        assert!(super::write_out(path, None, "x").is_err() && !p.exists());
        std::fs::write(&p, "").unwrap();
        let id = super::file_id(&std::fs::File::open(&p).unwrap());
        super::write_out(path, id.as_deref(), "new").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "new");
        // The right file, but not empty (a forged launch naming some file by its id): left as it is.
        assert!(super::write_out(path, id.as_deref(), "x").is_err());
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
            was_extra: vec![Addr { address: "192.168.1.60".into(), subnet: "255.255.255.0".into() }, Addr { address: "172.16.0.9".into(), subnet: "255.255.0.0".into() }], was_gws: vec![], was_metric: String::new(), was_first: String::new() };
        let got: Vec<String> = commands(&r).unwrap().iter().map(|c| c.join(" ")).collect();
        // The elevated copy gets the request as JSON and must build the same commands.
        let sent: Request = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
        assert_eq!(commands(&sent).unwrap(), commands(&r).unwrap(), "the Advanced lists survive the trip to the elevated copy");
        assert_eq!(got, [
            "octet:extra-address 12 del 172.16.0.9",
            "netsh interface ipv4 set interface interface=12 dhcpstaticipcoexistence=enabled",
            "octet:extra-address 12 add 10.9.0.1 16",
        ]);
        // Static: set address replaces all, so every extra address and gateway goes back on.
        let mut s = cfg();
        s.extra = vec![Addr { address: "192.168.1.60".into(), subnet: "255.255.255.0".into() }];
        s.gws = vec![Gw { gateway: "192.168.1.2".into(), metric: "20".into() }];
        s.gw_metric = "10".into();
        let r = Request::Apply { index: 12, name: "Wi-Fi".into(), cfg: s, ip: true, dns: false, extra: true, was_extra: vec![Addr { address: "192.168.1.60".into(), subnet: "255.255.255.0".into() }], was_gws: vec![], was_metric: String::new(), was_first: String::new() };
        let got: Vec<String> = commands(&r).unwrap().iter().map(|c| c.join(" ")).collect();
        assert_eq!(got, [
            "octet:address 12 192.168.1.50 255.255.255.0",
            "octet:gateway 12 add 192.168.1.1 10",
            "octet:extra-address 12 add 192.168.1.60 24",
            "octet:gateway 12 add 192.168.1.2 20",
        ]);
        // Bad Advanced values stop everything before any command.
        c.extra[0].subnet = "33".into();
        assert!(commands(&Request::Apply { index: 1, name: "a".into(), cfg: c, ip: false, dns: false, extra: true, was_extra: vec![], was_gws: vec![], was_metric: String::new(), was_first: String::new() }).is_err());
        let mut m = cfg();
        m.metric = "0".into();
        assert!(commands(&Request::Apply { index: 1, name: "a".into(), cfg: m, ip: false, dns: false, extra: true, was_extra: vec![], was_gws: vec![], was_metric: String::new(), was_first: String::new() }).is_err());
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn advanced_edits_and_failures_land_right() {
        use crate::adapters::{Addr, Gw};
        let run = |cfg: Cfg, ip: bool, dns: bool, was_extra: Vec<Addr>, was_gws: Vec<Gw>| -> Result<Vec<String>, String> {
            commands(&Request::Apply { index: 12, name: "Wi-Fi".into(), cfg, ip, dns, extra: true, was_extra, was_gws, was_metric: String::new(), was_first: String::new() }).map(|v| v.iter().map(|c| c.join(" ")).collect())
        };
        let addr = |a: &str, m: &str| Addr { address: a.into(), subnet: m.into() };
        let gw = |g: &str, m: &str| Gw { gateway: g.into(), metric: m.into() };
        // A mask or a metric changed on an entry that stays: it comes off and goes back on with the new value.
        let mut c = cfg();
        c.extra = vec![addr("192.168.1.60", "255.255.0.0")];
        c.gws = vec![gw("192.168.1.2", "20")];
        assert_eq!(run(c.clone(), false, false, vec![addr("192.168.1.60", "255.255.255.0")], vec![gw("192.168.1.2", "")]).unwrap(), [
            "octet:extra-address 12 del 192.168.1.60",
            "octet:gateway 12 del 192.168.1.2",
            "octet:extra-address 12 add 192.168.1.60 16",
            "octet:gateway 12 add 192.168.1.2 20",
        ]);
        // Unchanged: nothing to do.
        assert!(run(c.clone(), false, false, vec![addr("192.168.1.60", "16")], vec![gw("192.168.1.2", "20")]).unwrap().is_empty());
        // DNS right after the address, before the Advanced steps that might fail.
        let got = run(c.clone(), true, true, vec![], vec![]).unwrap();
        assert!(got[0].starts_with("octet:address") && got[2].starts_with("octet:set-dns"), "{got:?}");
        // A repeat would stop netsh halfway: refused before anything runs.
        let mut d = c.clone();
        d.extra.push(addr("192.168.1.60", "24"));
        assert!(run(d, false, false, vec![], vec![]).is_err());
        let mut d = c.clone();
        d.extra = vec![addr("192.168.1.50", "24")];   // the main address
        assert!(run(d, false, false, vec![], vec![]).is_err());
        let mut d = c.clone();
        d.gws = vec![gw("192.168.1.1", "")];   // the main gateway
        assert!(run(d, false, false, vec![], vec![]).is_err());
        // Back to DHCP: the old typed-in gateways stay off; a newly typed one goes on.
        let mut d = c.clone();
        (d.ip_auto, d.dns_auto) = (true, true);
        d.extra = vec![];
        d.gws = vec![gw("192.168.1.2", "20"), gw("192.168.1.3", "")];
        let got = run(d, true, false, vec![], vec![gw("192.168.1.2", "20")]).unwrap();
        assert_eq!(got, ["octet:address 12 dhcp", "octet:gateway 12 add 192.168.1.3 0"]);
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn first_gateway_set_kept_and_cleared() {
        let run = |first: &str, was: &str| -> Vec<String> {
            let mut c = cfg();
            c.first = first.into();
            let r = Request::Apply { index: 12, name: "Wi-Fi".into(), cfg: c, ip: false, dns: false, extra: true, was_extra: vec![], was_gws: vec![], was_metric: String::new(), was_first: was.into() };
            commands(&r).unwrap().iter().map(|c| c.join(" ")).collect()
        };
        assert_eq!(run("192.168.1.2", ""), ["octet:first 12 192.168.1.2"]);
        assert!(run("192.168.1.2", "192.168.1.2").is_empty());
        assert_eq!(run("", "192.168.1.2"), ["octet:first 12 none"]);
        let mut c = cfg();
        c.first = "nope".into();
        assert!(commands(&Request::Apply { index: 12, name: "a".into(), cfg: c, ip: false, dns: false, extra: true, was_extra: vec![], was_gws: vec![], was_metric: String::new(), was_first: String::new() }).is_err());
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn interface_metric_set_kept_and_back_to_automatic() {
        let run = |metric: &str, was: &str| -> Vec<String> {
            let mut c = cfg();
            c.metric = metric.into();
            let r = Request::Apply { index: 12, name: "Wi-Fi".into(), cfg: c, ip: false, dns: false, extra: true, was_extra: vec![], was_gws: vec![], was_metric: was.into(), was_first: String::new() };
            commands(&r).unwrap().iter().map(|c| c.join(" ")).collect()
        };
        assert_eq!(run("15", ""), ["octet:metric 12 15"]);
        assert!(run("15", "15").is_empty(), "unchanged: nothing sent");
        assert_eq!(run("", "15"), ["octet:metric 12 auto"], "clearing restores Automatic");
        assert!(run("", "").is_empty());
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn coexistence_off_after_the_last_extra_on_dhcp() {
        use crate::adapters::Addr;
        let run = |ip_auto: bool, extra: Vec<Addr>, was: Vec<Addr>| -> Vec<String> {
            let mut c = cfg();
            (c.ip_auto, c.extra) = (ip_auto, extra);
            let r = Request::Apply { index: 12, name: "Wi-Fi".into(), cfg: c, ip: false, dns: false, extra: true, was_extra: was, was_gws: vec![], was_metric: String::new(), was_first: String::new() };
            commands(&r).unwrap().iter().map(|c| c.join(" ")).collect()
        };
        let a = |s: &str| Addr { address: s.into(), subnet: "24".into() };
        assert_eq!(run(true, vec![], vec![a("192.168.1.60")]), [
            "octet:extra-address 12 del 192.168.1.60",
            "netsh interface ipv4 set interface interface=12 dhcpstaticipcoexistence=disabled",
        ]);
        assert_eq!(run(true, vec![a("192.168.1.61")], vec![a("192.168.1.60"), a("192.168.1.61")]),
            ["octet:extra-address 12 del 192.168.1.60"], "one still there: left on");
        assert_eq!(run(false, vec![], vec![a("192.168.1.60")]),
            ["octet:extra-address 12 del 192.168.1.60"], "static: not touched");
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn builds_static_apply_by_index() {
        let c = commands(&apply(cfg(), true, true)).unwrap();
        assert_eq!(
            c[0].join(" "),
            "octet:address 12 192.168.1.50 255.255.255.0"
        );
        assert_eq!(c[1].join(" "), "octet:gateway 12 add 192.168.1.1 0");
        assert_eq!(c[2].join(" "), "octet:set-dns 12 1.1.1.1,8.8.8.8 noregister");
        assert_eq!(c.len(), 3);
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
        assert_eq!(only_ip[0], cmd(&["octet:address", "12", "dhcp"]));
        c.ip_auto = false;
        c.dns_auto = true;
        assert!(commands(&apply(c.clone(), true, true)).is_err(), "static IP with automatic DNS");
        c.dns_auto = false;
        c.address = "300.1.1.1".into();
        assert!(commands(&apply(c, true, false)).is_err());
        assert!(commands(&Request::Renew { index: 1, name: "a\" & calc".into(), cycle: false }).is_err());
        assert!(commands(&Request::Release { index: 1, name: "*".into() }).is_err());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn builds_networksetup_by_service_name() {
        let got = |r: &Request| commands(r).map(|c| c.iter().map(|c| c.join(" ")).collect::<Vec<_>>());
        let wifi = |cfg: Cfg, ip, dns| Request::Apply { index: 12, name: "Wi-Fi".into(), cfg, ip, dns, extra: false, was_extra: vec![], was_gws: vec![], was_metric: String::new(), was_first: String::new() };
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
        assert_eq!(got(&Request::Release { index: 4, name: "Wi-Fi".into() }).unwrap(), ["networksetup -setv4off Wi-Fi"]);
        assert_eq!(got(&Request::Renew { index: 4, name: "Wi-Fi".into(), cycle: false }).unwrap(), ["octet:lease 4 renew"]);
        assert_eq!(got(&Request::Renew { index: 4, name: "Wi-Fi".into(), cycle: true }).unwrap(), ["networksetup -setv4off Wi-Fi", "octet:lease 4 renew"]);
        assert_eq!(got(&Request::FlushDns).unwrap(), ["dscacheutil -flushcache", "killall -HUP mDNSResponder"]);
    }

    /// Advanced on a Mac: each extra address is a service of its own beside the first, with the gateway on its network.
    #[test]
    #[cfg(target_os = "macos")]
    fn mac_advanced_addresses_are_services() {
        use crate::adapters::{Addr, Gw};
        let got = |r: &Request| commands(r).map(|c| c.iter().map(|c| c.join(" ")).collect::<Vec<_>>());
        let adv = |cfg: Cfg, was_extra: Vec<Addr>| Request::Apply { index: 12, name: "Wi-Fi".into(), cfg, ip: false, dns: false, extra: true, was_extra, was_gws: vec![], was_metric: String::new(), was_first: String::new() };
        let addr = |a: &str, m: &str| Addr { address: a.into(), subnet: m.into() };
        let mut c = cfg();   // 192.168.1.50/24 via 192.168.1.1
        c.extra = vec![addr("10.0.0.2", "24")];
        c.gws = vec![Gw { gateway: "10.0.0.1".into(), metric: String::new() }];
        assert_eq!(got(&adv(c.clone(), vec![])).unwrap(), [
            "networksetup -duplicatenetworkservice Wi-Fi Wi-Fi + 10.0.0.2",
            "networksetup -setmanual Wi-Fi + 10.0.0.2 10.0.0.2 255.255.255.0 10.0.0.1",
            "networksetup -setdnsservers Wi-Fi + 10.0.0.2 Empty",
            "networksetup -setv6off Wi-Fi + 10.0.0.2",
        ]);
        // Already there: set again (its gateway may have changed), not made again. Gone from the list: removed.
        assert_eq!(got(&adv(c.clone(), vec![addr("10.0.0.2", "255.255.255.0"), addr("172.16.0.9", "255.255.0.0")])).unwrap(), [
            "networksetup -removenetworkservice Wi-Fi + 172.16.0.9",
            "networksetup -setmanual Wi-Fi + 10.0.0.2 10.0.0.2 255.255.255.0 10.0.0.1",
        ]);
        // A gateway on no address's network, two on one address, a metric, a repeat: refused before anything runs.
        let mut x = c.clone();
        x.gws = vec![Gw { gateway: "172.31.0.1".into(), metric: String::new() }];
        assert!(commands(&adv(x, vec![])).is_err(), "gateway off every network");
        let mut x = c.clone();
        x.gws.push(Gw { gateway: "10.0.0.254".into(), metric: String::new() });
        assert!(commands(&adv(x, vec![])).is_err(), "two gateways for one address");
        let mut x = c.clone();
        x.gws[0].metric = "10".into();
        assert!(commands(&adv(x, vec![])).is_err(), "metric");
        let mut x = c.clone();
        x.extra.push(addr("192.168.1.50", "24"));
        assert!(commands(&adv(x, vec![])).is_err(), "the first address again");
        // On DHCP the lease's router stays the lease's; the extra still gets its own.
        let mut x = c;
        x.ip_auto = true;
        assert_eq!(got(&adv(x, vec![addr("10.0.0.2", "24")])).unwrap(), ["networksetup -setmanual Wi-Fi + 10.0.0.2 10.0.0.2 255.255.255.0 10.0.0.1"]);
        // Sharing: this Mac takes the .1.
        let share = |scope: &str| serde_json::from_str::<Request>(&format!(r#"{{"op":"share","public":11,"private":7,"on":true,"scope":"{scope}"}}"#)).unwrap();
        assert_eq!(got(&share("192.168.9.1")).unwrap(), ["octet:share 11 7 on 192.168.9.1"]);
        assert!(commands(&share("192.168.9.20")).is_err());
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
