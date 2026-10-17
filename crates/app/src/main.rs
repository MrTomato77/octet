#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;

mod adapters;
mod app;
mod ipv4_input;
mod language;
mod session;
mod storage;
mod theme;

fn main() -> eframe::Result {
    let store = storage::Store::open_default();
    let mut thai = store.state.thai;
    let mut scale = 1.0;
    let mut size = store.state.size;
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--thai" => thai = true,
            "--english" => thai = false,
            "--scale" => {
                scale = iter
                    .next()
                    .and_then(|s| s.parse::<f32>().ok())
                    .filter(|v| (0.75..=2.0).contains(v))
                    .unwrap_or(1.0);
            }
            "--compact" => size = [700.0, 470.0],
            "--demo" => {}
            _ => {}
        }
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(size)
            .with_min_inner_size([700.0, 470.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Simple IP v2",
        options,
        Box::new(move |cc| Ok(Box::new(app::SimpleIpApp::new(cc, thai, scale, store)))),
    )
}
