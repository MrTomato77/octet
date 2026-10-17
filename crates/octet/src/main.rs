#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod adapters;
mod change;

use change::Request;
use std::sync::atomic::{AtomicBool, Ordering};

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

/// Blocking work (PowerShell, UAC, netsh) off the async runtime.
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

#[tauri::command]
fn open_ncpa() -> Result<(), String> {
    hidden("control").arg("ncpa.cpl").spawn().map(|_| ()).map_err(|e| e.to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let [_, flag, request, out] = args.as_slice() {
        if flag == "--elevated" {
            std::process::exit(change::run_elevated(request, out));
        }
    }
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![read_adapters, change, flush_dns, open_ncpa])
        .run(tauri::generate_context!())
        .expect("failed to start Octet");
}
