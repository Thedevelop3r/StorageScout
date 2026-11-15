#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod analysis;
mod app;
mod demo;
mod model;
mod scanner;
mod system;
mod ui;
mod util;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("PC Scanner")
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([900.0, 560.0])
            .with_icon(egui::IconData {
                rgba: include_bytes!("../assets/icon_64.rgba").to_vec(),
                width: 64,
                height: 64,
            }),
        ..Default::default()
    };
    // Optional: folders/drives passed on the command line are scanned right away.
    // `--view=<name>` opens a specific view (overview, explorer, treemap, groups, ...).
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    let mut view: Option<String> = None;
    // `--demo` shows a fictional computer instead of scanning this one.
    let mut demo = false;
    for arg in std::env::args_os().skip(1) {
        let s = arg.to_string_lossy();
        if s == "--demo" {
            demo = true;
        } else if let Some(v) = s.strip_prefix("--view=") {
            view = Some(v.to_lowercase());
        } else {
            let p = std::path::PathBuf::from(&arg);
            if p.is_dir() {
                paths.push(p);
            }
        }
    }
    eframe::run_native(
        "PC Scanner",
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::PcScannerApp::new(
                cc,
                paths,
                view.as_deref(),
                demo,
            )))
        }),
    )
}
