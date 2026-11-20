mod controls;

#[cfg(not(target_arch = "wasm32"))]
#[path = "gui/native.rs"]
mod platform;
#[cfg(target_arch = "wasm32")]
#[path = "gui/browser.rs"]
mod platform;

use eframe::egui;
use image::{DynamicImage, RgbImage};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use triklops::algo::{AlgorithmParams, Progress};

struct Reference {
    path: PathBuf,
    image: Arc<RgbImage>,
    texture: egui::TextureHandle,
}

#[derive(Default)]
pub struct TriKlopsApp {
    params: AlgorithmParams,
    reference: Option<Reference>,
    progress: Arc<Mutex<Progress>>,
    worker: Option<platform::Worker>,
    #[cfg(target_arch = "wasm32")]
    browser: platform::Browser,
    error: Option<String>,
    preview: Option<egui::TextureHandle>,
    preview_revision: usize,
    use_seed: bool,
    seed: String,
}

fn color_image(image: &RgbImage) -> egui::ColorImage {
    egui::ColorImage::from_rgb(
        [image.width() as usize, image.height() as usize],
        image.as_raw(),
    )
}

impl TriKlopsApp {
    fn set_reference(&mut self, ctx: &egui::Context, path: PathBuf, image: DynamicImage) {
        let size = self.params.image_size;
        let image = image
            .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
            .to_rgb8();
        let texture = ctx.load_texture("reference", color_image(&image), Default::default());
        self.reference = Some(Reference {
            path,
            image: Arc::new(image),
            texture,
        });
        self.reset_preview();
        self.error = None;
    }

    fn reset_preview(&mut self) {
        self.progress = Default::default();
        self.preview = None;
        self.preview_revision = 0;
    }

    fn start(&mut self, ctx: egui::Context) {
        self.reset_preview();
        self.error = None;
        self.start_worker(ctx);
    }

    fn update_preview(&mut self, ctx: &egui::Context, p: &Progress) {
        if self.preview_revision == p.revision {
            return;
        }
        let Some(canvas) = &p.canvas else { return };
        let image = if let Some(candidate) = p.candidate {
            let mut preview = (**canvas).clone();
            candidate.paint(&mut preview);
            color_image(&preview)
        } else {
            color_image(canvas)
        };
        match &mut self.preview {
            Some(texture) => texture.set(image, Default::default()),
            None => self.preview = Some(ctx.load_texture("preview", image, Default::default())),
        }
        self.preview_revision = p.revision;
    }

    fn show_images(&self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let width = (available.x - ui.spacing().item_spacing.x).max(0.0);
        let reference_size = (width / 3.0).min(256.0).min(available.y);
        let output_size = (width - reference_size).min(available.y);
        ui.horizontal_top(|ui| {
            for (texture, size) in [
                (self.reference.as_ref().map(|r| &r.texture), reference_size),
                (self.preview.as_ref(), output_size),
            ] {
                let size = egui::Vec2::splat(size);
                if let Some(texture) = texture {
                    ui.add(egui::Image::new(texture).fit_to_exact_size(size));
                } else {
                    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                    ui.painter().rect_filled(rect, 0.0, egui::Color32::BLACK);
                }
            }
        });
    }
}

impl eframe::App for TriKlopsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_platform(ui.ctx());
        let progress = self.progress.lock().unwrap().clone();
        self.update_preview(ui.ctx(), &progress);
        self.show_controls(ui, &progress);
        egui::CentralPanel::default().show(ui, |ui| self.show_images(ui));
        #[cfg(not(target_arch = "wasm32"))]
        if self.worker.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(33));
        }
    }
}
