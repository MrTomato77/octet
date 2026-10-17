#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod adapters;
mod change;

use change::Request;

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

#[tauri::command]
async fn read_adapters() -> Result<Vec<adapters::Adapter>, String> {
    adapters::read()
}

#[tauri::command]
async fn change(request: Request) -> Result<(), String> {
    change::elevate(&request)
}

#[tauri::command]
async fn flush_dns() -> Result<(), String> {
    let o = hidden("ipconfig").arg("/flushdns").output().map_err(|e| e.to_string())?;
    if o.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&o.stdout).trim().to_string()) }
}

#[tauri::command]
fn open_ncpa() -> Result<(), String> {
    hidden("control").arg("ncpa.cpl").spawn().map(|_| ()).map_err(|e| e.to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let [_, flag, path] = args.as_slice() {
        if flag == "--elevated" {
            std::process::exit(change::run_elevated(path));
        }
    }
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![read_adapters, change, flush_dns, open_ncpa])
        .run(tauri::generate_context!())
        .expect("failed to start Octet");
}
