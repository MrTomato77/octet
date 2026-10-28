#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod adapters;
mod change;
mod net;
mod update;

use change::Request;
use std::net::Ipv4Addr;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};

/// A child process without a console window flashing up.
pub fn hidden(program: &str) -> std::process::Command {
    let mut c = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c
}

/// Blocking work (PowerShell, UAC, netsh, ICMP) off the async runtime.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn read_adapters() -> Result<Vec<adapters::Adapter>, String> {
    blocking(adapters::read).await
}

/// One change at a time: a second click must not raise a second UAC prompt.
static CHANGING: AtomicBool = AtomicBool::new(false);

#[tauri::command]
async fn change(request: Request) -> Result<(), String> {
    if CHANGING.swap(true, Ordering::SeqCst) {
        return Err("Another change is still running".into());
    }
    let result = blocking(move || change::elevate(&request)).await;
    CHANGING.store(false, Ordering::SeqCst);
    result
}

#[tauri::command]
async fn flush_dns() -> Result<(), String> {
    blocking(|| if net::flush_dns() { Ok(()) } else { Err("Could not flush the DNS cache".into()) }).await
}

/// Before Apply: does another device already answer ARP for this address?
#[tauri::command]
async fn who_has(address: String) -> Result<Option<String>, String> {
    let ip: Ipv4Addr = address.parse().map_err(|_| "Not an IPv4 address".to_string())?;
    blocking(move || Ok(net::arp(ip))).await
}

/// Round trip in ms, or None. With `source` (the adapter's address) it goes out through that adapter.
#[tauri::command]
async fn ping(address: String, source: Option<String>) -> Result<Option<u32>, String> {
    let ip: Ipv4Addr = address.parse().map_err(|_| "Not an IPv4 address".to_string())?;
    let from = source.and_then(|s| s.parse().ok());
    blocking(move || Ok(net::ping(ip, from, 1500))).await
}

/// A TCP connect to `port`: {"open": ms}, {"closed": ms} (refused: the device is there), or null (nothing answered).
#[tauri::command]
/// `timeout` in ms, 1500 when not given (Scan's port check asks for less: a LAN device answers in a few ms).
async fn tcping(address: String, port: u16, source: Option<String>, timeout: Option<u32>) -> Result<Option<net::Tcp>, String> {
    let ip: Ipv4Addr = address.parse().map_err(|_| "Not an IPv4 address".to_string())?;
    if port == 0 {
        return Err("Port must be 1 to 65535".into());
    }
    let from = source.and_then(|s| s.parse().ok());
    blocking(move || Ok(net::tcping(ip, port, from, timeout.unwrap_or(1500).clamp(100, 1500)))).await
}

/// Opens a device's web page (Scan, Check ports) in the default browser. Only an IPv4 address and a port
/// go into the URL, so nothing else can be opened through it.
#[tauri::command]
fn open_device(address: String, port: u16, tls: bool) -> Result<(), String> {
    let ip: Ipv4Addr = address.parse().map_err(|_| "Not an IPv4 address".to_string())?;
    let url = format!("{}://{ip}:{port}/", if tls { "https" } else { "http" });
    hidden("explorer").arg(url).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Scan: who holds this address, seen from the adapter (`index`, its own `source` address).
#[tauri::command]
async fn scan_host(address: String, source: Option<String>, index: u32) -> Result<Option<net::Host>, String> {
    let ip: Ipv4Addr = address.parse().map_err(|_| "Not an IPv4 address".to_string())?;
    let from = source.and_then(|s| s.parse().ok());
    blocking(move || Ok(net::scan_host(ip, from, index))).await
}

/// Settings, Updates: the newer version on GitHub, or None.
#[tauri::command]
async fn update_check() -> Result<Option<String>, String> {
    blocking(update::check).await
}

/// Downloads and installs `version` (signature checked), sending "update-progress" (0 to 1) as it comes.
#[tauri::command]
async fn update_install(app: tauri::AppHandle, version: String) -> Result<(), String> {
    blocking(move || update::install(&version, |p| { let _ = app.emit("update-progress", p); })).await
}

/// Starts the new exe and closes this one.
#[tauri::command]
fn update_restart(app: tauri::AppHandle) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::process::Command::new(exe).arg("--updated").spawn().map_err(|e| e.to_string())?;
    app.exit(0);
    Ok(())
}

/// The gateway's name, if it gives one.
#[tauri::command]
async fn hostname(address: String) -> Result<Option<String>, String> {
    let ip: Ipv4Addr = address.parse().map_err(|_| "Not an IPv4 address".to_string())?;
    blocking(move || Ok(net::hostname(ip))).await
}

#[tauri::command]
fn computer_name() -> String {
    net::computer_name()
}

/// Info: the public address, only when the user asks for it.
#[tauri::command]
async fn public_ip() -> Result<net::Public, String> {
    blocking(net::public_ip).await
}

/// Who made the card behind a MAC, from the built-in IEEE list.
#[tauri::command]
fn vendor(mac: String) -> Option<&'static str> {
    net::vendor(&mac)
}

#[tauri::command]
fn open_ncpa() -> Result<(), String> {
    hidden("control").arg("ncpa.cpl").spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Opens one of Octet's own pages in the default browser. A fixed list, so the
/// page can't be made to launch anything else.
#[tauri::command]
fn open_link(which: String) -> Result<(), String> {
    let url = match which.as_str() {
        "repo" => "https://github.com/MrTomato77/octet",
        "releases" => "https://github.com/MrTomato77/octet/releases",
        "notices" => "https://github.com/MrTomato77/octet/blob/main/THIRD-PARTY-NOTICES.md",
        "simpleip" => "https://github.com/KurtisLiggett/Simple-IP-Config",
        _ => return Err("Unknown link".into()),
    };
    hidden("explorer").arg(url).spawn().map(|_| ()).map_err(|e| e.to_string())
}

// ---- "Always run as administrator": a marker file in %APPDATA%\Octet ----

fn admin_marker() -> Option<std::path::PathBuf> {
    std::env::var_os("APPDATA").map(|d| std::path::Path::new(&d).join("Octet").join("run-as-admin"))
}

fn always_admin() -> bool {
    admin_marker().is_some_and(|p| p.exists())
}

#[derive(serde::Serialize)]
struct AdminState {
    elevated: bool,
    always: bool,
}

#[tauri::command]
fn admin_state() -> AdminState {
    AdminState { elevated: net::elevated(), always: always_admin() }
}

#[tauri::command]
fn set_always_admin(on: bool) -> Result<(), String> {
    let p = admin_marker().ok_or("No APPDATA folder")?;
    if on {
        std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&p, b"").map_err(|e| e.to_string())
    } else {
        match std::fs::remove_file(&p) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }
}

/// Start an elevated copy now and close this one. Err if UAC was declined.
#[tauri::command]
fn restart_as_admin(app: tauri::AppHandle) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    change::run_as_admin(&exe, "--restarted", false)?;
    app.exit(0);
    Ok(())
}

/// The page has painted its first frame: show the window now, so it never opens white.
#[tauri::command]
fn ready(window: tauri::WebviewWindow) {
    let _ = window.show();
    let _ = window.set_focus();
}

static APP: OnceLock<tauri::AppHandle> = OnceLock::new();

/// poc/speedtest: runs the Cloudflare SDK POC when OCTET_POC_SPEED is set (never in a release).
#[tauri::command]
fn poc_speed() -> bool {
    let on = std::env::var_os("OCTET_POC_SPEED").is_some();
    poc_log(format!("poc_speed asked: {on}"));
    on
}
/// poc/speedtest: one line to %TEMP%\octet-poc.log.
#[tauri::command]
fn poc_log(line: String) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(std::env::temp_dir().join("octet-poc.log")) {
        let _ = writeln!(f, "{line}");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let [_, flag, request, out] = args.as_slice() {
        if flag == "--elevated" {
            std::process::exit(change::run_elevated(request, out));
        }
    }
    update::tidy();
    let restarted = args.iter().any(|a| a == "--restarted");
    // After an update: wait for the old copy to close, then start as usual (elevated again if set to).
    if args.iter().any(|a| a == "--updated") {
        std::thread::sleep(std::time::Duration::from_millis(700));
    }
    if restarted {
        // Let the copy that started us close first, or single-instance would hand off to it.
        std::thread::sleep(std::time::Duration::from_millis(700));
    } else if always_admin() && !args.iter().any(|a| a == "--admin") && !net::elevated() {
        if let Ok(exe) = std::env::current_exe() {
            // No window or single-instance lock exists yet, so the elevated copy needn't wait.
            if change::run_as_admin(&exe, "--admin", false).is_ok() {
                return;
            }
        } // UAC declined: carry on without admin rights.
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .setup(|app| {
            let _ = APP.set(app.handle().clone());
            // Taskbar and Alt+Tab take the window's own icon; give it the large one explicitly.
            if let (Some(w), Ok(icon)) = (app.get_webview_window("main"), tauri::image::Image::from_bytes(include_bytes!("../icons/128x128@2x.png"))) {
                let _ = w.set_icon(icon);
            }
            // If the page never calls ready, show the window anyway.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(3));
                if let Some(w) = handle.get_webview_window("main") {
                    if !w.is_visible().unwrap_or(true) { let _ = w.show(); }
                }
            });
            // Windows calls this on its own thread for every interface or address change.
            net::watch(|| {
                if let Some(app) = APP.get() {
                    let _ = app.emit("net-changed", ());
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            read_adapters, change, flush_dns, who_has, ping, tcping, open_device, scan_host, hostname, vendor, computer_name, public_ip, open_ncpa, open_link, update_check, update_install, update_restart, ready, admin_state, set_always_admin, restart_as_admin, poc_speed, poc_log
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Octet");
}
