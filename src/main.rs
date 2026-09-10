// disables windows terminal pop-up in background
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod model;
mod parser;

use app::LogViewerApp;

/// main entry point to log viewer app
fn main() -> eframe::Result<()> {
    //define the options for application GUI window
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_title("WAKO Log Viewer"),
        ..Default::default()
    };

    //main entry point to GUI
    eframe::run_native(
        "WAKO Point Panel Log Viewer",
        options,
        Box::new(|_cc| Ok(Box::new(LogViewerApp::default()))),
    )
}
