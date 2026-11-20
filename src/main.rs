#![windows_subsystem = "windows"]
mod gui;

use crate::gui::TriKlopsApp;
use eframe::egui;

fn main() -> eframe::Result {
    let app_name = "Tri-Klops";
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([790.0, 360.0])
            .with_title(app_name),
        ..Default::default()
    };
    eframe::run_native(
        app_name,
        options,
        Box::new(|_| Ok(Box::<TriKlopsApp>::default())),
    )
}
