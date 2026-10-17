#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod adapters;
mod change;
mod net;

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
    blocking(|| {
        let o = hidden("ipconfig").arg("/flushdns").output().map_err(|e| e.to_string())?;
        if o.status.success() { Ok(()) } else { Err(change::oem_text(&o.stdout).trim().to_string()) }
    })
    .await
}

/// Before Apply: does another device already answer ARP for this address?
#[tauri::command]
async fn who_has(address: String) -> Result<Option<String>, String> {
    let ip: Ipv4Addr = address.parse().map_err(|_| "Not an IPv4 address".to_string())?;
    blocking(move || Ok(net::arp(ip))).await
}

/// After Apply: round trip in ms, or None.
#[tauri::command]
async fn ping(address: String) -> Result<Option<u32>, String> {
    let ip: Ipv4Addr = address.parse().map_err(|_| "Not an IPv4 address".to_string())?;
    blocking(move || Ok(net::ping(ip, 1500))).await
}

/// The gateway's name, if it gives one.
#[tauri::command]
async fn hostname(address: String) -> Result<Option<String>, String> {
    let ip: Ipv4Addr = address.parse().map_err(|_| "Not an IPv4 address".to_string())?;
    blocking(move || Ok(net::hostname(ip))).await
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

static APP: OnceLock<tauri::AppHandle> = OnceLock::new();

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let [_, flag, request, out] = args.as_slice() {
        if flag == "--elevated" {
            std::process::exit(change::run_elevated(request, out));
        }
    }
    let restarted = args.iter().any(|a| a == "--restarted");
    if restarted {
        // Let the copy that started us close first, or single-instance would hand off to it.
        std::thread::sleep(std::time::Duration::from_millis(700));
    } else if always_admin() && !net::elevated() {
        if let Ok(exe) = std::env::current_exe() {
            if change::run_as_admin(&exe, "--restarted", false).is_ok() {
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
            // Windows calls this on its own thread for every interface or address change.
            net::watch(|| {
                if let Some(app) = APP.get() {
                    let _ = app.emit("net-changed", ());
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            read_adapters, change, flush_dns, who_has, ping, hostname, open_ncpa, open_link, admin_state, set_always_admin, restart_as_admin
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Octet");
}
