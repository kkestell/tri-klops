use super::{TriKlopsApp, decode_reference};
use eframe::egui;
use image::ImageReader;
use std::thread::{self, JoinHandle};
use triklops::algo::run_algorithm;

pub(super) type Worker = JoinHandle<std::io::Result<()>>;

impl TriKlopsApp {
    pub(super) fn load_reference(&mut self, _ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif", "webp"])
            .pick_file()
        else {
            return;
        };
        match ImageReader::open(&path)
            .map_err(image::ImageError::from)
            .and_then(decode_reference)
        {
            Ok(image) => self.set_reference(path, image),
            Err(e) => self.error = Some(format!("Could not load {}: {e}", path.display())),
        }
    }

    pub(super) fn start_worker(&mut self, ctx: egui::Context) {
        let reference = self.reference.as_ref().unwrap();
        let image = reference.image.clone();
        let output = reference.path.with_extension("svg");
        let progress = self.progress.clone();
        let params = self.params.clone();
        self.worker = Some(thread::spawn(move || {
            let result = run_algorithm(&params, &image, &output, &progress);
            ctx.request_repaint();
            result
        }));
    }

    pub(super) fn stop_worker(&mut self) {
        self.progress.lock().unwrap().should_stop = true;
    }

    pub(super) fn poll_platform(&mut self) {
        if self.worker.as_ref().is_some_and(|w| w.is_finished()) {
            self.error = match self.worker.take().unwrap().join() {
                Ok(Ok(())) => None,
                Ok(Err(err)) => Some(format!("Could not save SVG: {err}")),
                Err(_) => Some("Image generation failed.".into()),
            };
        }
    }
}
