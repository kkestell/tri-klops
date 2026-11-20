#![windows_subsystem = "windows"]
mod gui;

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
use crate::gui::TriKlopsApp;
#[cfg(not(target_arch = "wasm32"))]
use eframe::egui;

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let app_name = "Tri-Klops";
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([832.0, 480.0])
            .with_title(app_name),
        ..Default::default()
    };
    eframe::run_native(
        app_name,
        options,
        Box::new(|_| Ok(Box::<TriKlopsApp>::default())),
    )
}
