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
    use_seed: bool,
    seed: String,
}

fn color_image(image: &RgbImage) -> egui::ColorImage {
    egui::ColorImage::from_rgb(
        [image.width() as usize, image.height() as usize],
        image.as_raw(),
    )
}

fn paint_preview(ui: &egui::Ui, rect: egui::Rect, scale: f32, progress: &Progress) {
    let painter = ui.painter().with_clip_rect(rect);
    let Some(drawing) = &progress.drawing else {
        painter.rect_filled(rect, 0.0, egui::Color32::BLACK);
        return;
    };

    let color = |[r, g, b]: [u8; 3]| egui::Color32::from_rgb(r, g, b);
    painter.rect_filled(rect, 0.0, color(drawing.background));
    // A mesh keeps sharp corners from growing long fringes during path tessellation.
    let mut mesh = egui::Mesh::default();
    for triangle in drawing.triangles.iter().copied().chain(progress.candidate) {
        let first = mesh.vertices.len() as u32;
        for [x, y] in triangle.vertices {
            let point = rect.min + egui::vec2(x as f32, y as f32) * scale;
            mesh.colored_vertex(point, color(triangle.color));
        }
        mesh.add_triangle(first, first + 1, first + 2);
    }
    painter.add(egui::Shape::mesh(mesh));
}

impl TriKlopsApp {
    fn set_reference(&mut self, ctx: &egui::Context, path: PathBuf, image: DynamicImage) {
        let size = self.params.image_size;
        let image = image
            .resize(size, size, image::imageops::FilterType::Lanczos3)
            .to_rgb8();
        let texture = ctx.load_texture("reference", color_image(&image), Default::default());
        self.reference = Some(Reference {
            path,
            image: Arc::new(image),
            texture,
        });
        self.progress = Default::default();
        self.error = None;
    }

    fn start(&mut self, ctx: egui::Context) {
        self.progress = Default::default();
        self.error = None;
        self.start_worker(ctx);
    }

    fn show_images(&self, ui: &mut egui::Ui, progress: &Progress) {
        let dimensions = self.reference.as_ref().map_or_else(
            || egui::Vec2::splat(self.params.image_size as f32),
            |reference| reference.texture.size_vec2(),
        );
        let aspect = dimensions.x / dimensions.y;
        let available = ui.available_size();
        let width = (available.x - ui.spacing().item_spacing.x).max(0.0);
        let reference_width = (width / 3.0)
            .min(256.0)
            .min(available.y.min(256.0) * aspect);
        let output_width = (width - reference_width).min(available.y * aspect);
        ui.horizontal_top(|ui| {
            let reference_size = egui::vec2(reference_width, reference_width / aspect);
            if let Some(reference) = &self.reference {
                ui.add(egui::Image::new(&reference.texture).fit_to_exact_size(reference_size));
            } else {
                let (rect, _) = ui.allocate_exact_size(reference_size, egui::Sense::hover());
                ui.painter().rect_filled(rect, 0.0, egui::Color32::BLACK);
            }
            let output_size = egui::vec2(output_width, output_width / aspect);
            let (rect, _) = ui.allocate_exact_size(output_size, egui::Sense::hover());
            paint_preview(ui, rect, rect.width() / dimensions.x, progress);
        });
    }
}

impl eframe::App for TriKlopsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_platform(ui.ctx());
        let progress = self.progress.lock().unwrap().clone();
        self.show_controls(ui, &progress);
        egui::CentralPanel::default().show(ui, |ui| self.show_images(ui, &progress));
        #[cfg(not(target_arch = "wasm32"))]
        if self.worker.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(33));
        }
    }
}
