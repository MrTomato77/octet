//! The macOS menu bar item: Octet's icon, and a menu with what the network is doing now and the quick changes
//! (DNS, Renew, Flush DNS, stopping Internet Sharing), then the page's own Speed test, Settings and update, and Quit.
//! The menu is built again whenever the network changes, so it always shows what is there.
//! Changes go through the same requests and password prompt as the page's, one at a time with them.

use crate::adapters::Adapter;
use crate::change::Request;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Wry};

/// The page's DNS presets (ui/index.html, `presets`), in the same order.
const PRESETS: [(&str, &str, &str); 4] = [
    ("Google", "8.8.8.8", "8.8.4.4"),
    ("Cloudflare", "1.1.1.1", "1.0.0.1"),
    ("Quad9", "9.9.9.9", "149.112.112.112"),
    ("AdGuard", "94.140.14.14", "94.140.15.15"),
];

/// The page's update state ("idle", "new", "getting", "ready", ...) and the version it found. The page owns the
/// update (it only asks GitHub when told to); the menu shows what it says, through `set_update`.
static UPDATE: Mutex<(String, String)> = Mutex::new((String::new(), String::new()));

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id("main")
        .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/tray-mac.png"))?)
        .icon_as_template(true)   // black: macOS tints it for light and dark menu bars
        .tooltip("Octet")
        .menu(&build(app)?)
        .show_menu_on_left_click(true)   // a click opens the menu, as menu bar items do on a Mac
        .on_menu_event(|app, e| act(app, e.id.as_ref()))
        .build(app)?;
    Ok(())
}

pub fn set_update(app: &AppHandle, state: String, version: String) {
    *UPDATE.lock().unwrap() = (state, version);
    refresh(app);
}

/// Builds the menu again off the main thread (reading the adapters takes some ms). A burst of network changes
/// (a cable in: link, address, DNS, route) makes one rebuild, a moment after the first.
pub fn refresh(app: &AppHandle) {
    static PENDING: AtomicBool = AtomicBool::new(false);
    if PENDING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(400));
        PENDING.store(false, Ordering::SeqCst);
        if let (Some(t), Ok(m)) = (app.tray_by_id("main"), build(&app)) {
            let _ = t.set_menu(Some(m));
        }
    });
}

/// The adapter that reaches the internet: the list's first (connected, with a gateway: see `adapters::read`).
fn online(list: &[Adapter]) -> Option<&Adapter> {
    list.first().filter(|a| a.state == "up" && !a.cfg.gateway.is_empty())
}

fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let list = crate::adapters::read().unwrap_or_default();
    let net = online(&list);
    let item = |id: &str, text: &str, on: bool, keys: Option<&str>| MenuItem::with_id(app, id, text, on, keys);
    let sep = || PredefinedMenuItem::separator(app);
    let m = Menu::new(app)?;
    // What the network is doing, greyed: a line to read, not to click.
    let status = match net {
        Some(a) => [a.name.as_str(), a.ssid.as_str(), a.cfg.address.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" · "),
        None => "Not connected".into(),
    };
    m.append(&item("status", &status, false, None)?)?;
    m.append(&sep()?)?;
    if let Some(a) = net {
        let dns = Submenu::with_id(app, "dns", "DNS", true)?;
        dns.append(&CheckMenuItem::with_id(app, "dns:auto", "Automatic", true, a.cfg.dns_auto, None::<&str>)?)?;
        dns.append(&sep()?)?;
        let mut known = a.cfg.dns_auto;
        for (i, (name, d1, d2)) in PRESETS.iter().enumerate() {
            let on = !a.cfg.dns_auto && a.cfg.dns1 == *d1 && a.cfg.dns2 == *d2;
            known |= on;
            dns.append(&CheckMenuItem::with_id(app, format!("dns:{i}"), format!("{name}    {d1}"), true, on, None::<&str>)?)?;
        }
        // Servers of your own (the router's, say): shown as what is in use, not offered as a choice.
        if !known {
            let own = [a.cfg.dns1.as_str(), a.cfg.dns2.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(", ");
            dns.append(&sep()?)?;
            dns.append(&CheckMenuItem::with_id(app, "dns-own", format!("Manual    {own}"), false, true, None::<&str>)?)?;
        }
        m.append(&dns)?;
        if a.cfg.ip_auto && !a.ppp {
            m.append(&item("renew", "Renew DHCP", true, None)?)?;
        }
    }
    m.append(&item("flush", "Flush DNS", true, None)?)?;
    let share = crate::mac::share_state();
    if share.on {
        let to = list.iter().find(|a| share.private.contains(&a.index)).map_or("another adapter", |a| a.name.as_str());
        m.append(&CheckMenuItem::with_id(app, "share-stop", format!("Sharing to {to}"), true, true, None::<&str>)?)?;
    }
    m.append(&item("speed", "Speed test…", net.is_some(), None)?)?;
    m.append(&sep()?)?;
    m.append(&item("open", "Open Octet", true, None)?)?;
    m.append(&item("settings", "Settings…", true, Some("CmdOrCtrl+,"))?)?;
    // An update, once the page has found one (it looks only when asked, or at start with Auto-check on).
    let (state, v) = UPDATE.lock().unwrap().clone();
    match state.as_str() {
        "new" => {
            m.append(&sep()?)?;
            m.append(&item("upd-say", &format!("Octet {v} is available"), false, None)?)?;
            m.append(&item("update", "Download update", true, None)?)?;
        }
        "getting" => {
            m.append(&sep()?)?;
            m.append(&item("upd-say", &format!("Downloading Octet {v}…"), false, None)?)?;
        }
        "ready" => {
            m.append(&sep()?)?;
            m.append(&item("upd-say", "An update is ready", false, None)?)?;
            m.append(&item("update", "Restart to update", true, None)?)?;
        }
        _ => {}
    }
    m.append(&sep()?)?;
    m.append(&item("quit", "Quit Octet", true, Some("CmdOrCtrl+Q"))?)?;
    Ok(m)
}

fn act(app: &AppHandle, id: &str) {
    match id {
        "open" => crate::bring(app),
        // The page's own: shown, then asked (it opens the speed test or Settings itself).
        "settings" | "speed" => {
            crate::bring(app);
            let _ = app.emit("tray", id);
        }
        // The page knows where its update is (found, or downloaded) and takes the next step.
        "update" => {
            let _ = app.emit("tray", "update");
        }
        "quit" => app.exit(0),
        // As net::flush_dns does on a Mac, but keeping why it failed (a closed password prompt says nothing).
        "flush" => change(app, || crate::change::elevate(&Request::FlushDns)),
        "renew" => with_online(app, |a| crate::change::elevate(&Request::Renew { index: a.index, name: a.name.clone(), cycle: false })),
        "share-stop" => {
            let app2 = app.clone();
            change(app, move || {
                let s = crate::mac::share_state();
                let private = *s.private.first().ok_or("Internet Sharing has no adapter to stop")?;
                crate::change::elevate(&Request::Share { public: s.public, private, on: false, scope: None, restore: None })?;
                let _ = app2.emit("tray", "share-stopped");   // the page forgets its record of it
                Ok(())
            })
        }
        dns if dns.starts_with("dns:") => {
            let preset = dns.strip_prefix("dns:").and_then(|i| i.parse::<usize>().ok()).and_then(|i| PRESETS.get(i).copied());
            with_online(app, move |a| {
                let mut cfg = a.cfg.clone();
                (cfg.dns_auto, cfg.dns1, cfg.dns2) = match preset {
                    Some((_, d1, d2)) => (false, d1.into(), d2.into()),
                    None => (true, String::new(), String::new()),   // Automatic: from DHCP again
                };
                crate::change::elevate(&Request::Apply { index: a.index, name: a.name.clone(), cfg, ip: false, dns: true, extra: false,
                    was_extra: vec![], was_gws: vec![], was_metric: String::new(), was_first: String::new() })
            })
        }
        _ => {}
    }
}

/// A change to the adapter that reaches the internet, read afresh (the menu may be a while old).
fn with_online(app: &AppHandle, f: impl FnOnce(&Adapter) -> Result<(), String> + Send + 'static) {
    change(app, move || {
        let list = crate::adapters::read()?;
        f(online(&list).ok_or("Not connected")?)
    })
}

/// Runs a change on a thread of its own (the password prompt waits for you), one at a time with the page's.
/// A failure brings the window up and the page says why; either way the page and the menu read the network again.
fn change(app: &AppHandle, f: impl FnOnce() -> Result<(), String> + Send + 'static) {
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(e) = crate::one_change(f) && !e.starts_with("Cancelled") {   // a closed password prompt says nothing
            crate::bring(&app);
            let _ = app.emit("tray", serde_json::json!({ "error": e }));
        }
        let _ = app.emit("net-changed", ());
        refresh(&app);
    });
}
