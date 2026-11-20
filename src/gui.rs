mod controls;
mod parameters;

#[cfg(not(target_arch = "wasm32"))]
#[path = "gui/native.rs"]
mod platform;
#[cfg(target_arch = "wasm32")]
#[path = "gui/browser.rs"]
mod platform;

use eframe::egui;
use image::{DynamicImage, ImageDecoder, ImageReader, Limits, RgbImage};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;
use std::{
    io::{BufRead, Seek},
    path::PathBuf,
    sync::{Arc, Mutex},
};
use triklops::algo::{AlgorithmParams, Drawing, Progress, Triangle};

struct Reference {
    path: PathBuf,
    image: Arc<RgbImage>,
}

#[derive(Default)]
pub struct TriKlopsApp {
    params: AlgorithmParams,
    other_mode_settings: Option<parameters::SearchSettings>,
    reference: Option<Reference>,
    progress: Arc<Mutex<Progress>>,
    worker: Option<platform::Worker>,
    #[cfg(target_arch = "wasm32")]
    browser: platform::Browser,
    #[cfg(target_arch = "wasm32")]
    mobile_settings_open: bool,
    error: Option<String>,
    use_seed: bool,
    seed: String,
}

fn decode_reference(reader: ImageReader<impl BufRead + Seek>) -> image::ImageResult<DynamicImage> {
    let mut decoder = reader.into_decoder()?;
    // Preserve ImageReader::decode's allocation limit when reading orientation metadata.
    let mut limits = Limits::default();
    limits.reserve(decoder.total_bytes())?;
    decoder.set_limits(limits)?;

    let orientation = decoder.orientation()?;
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);
    Ok(image)
}

fn paint_preview(
    ui: &egui::Ui,
    rect: egui::Rect,
    scale: f32,
    drawing: &Drawing,
    candidate: Option<Triangle>,
) {
    let painter = ui.painter().with_clip_rect(rect);

    let color = |[r, g, b]: [u8; 3]| egui::Color32::from_rgb(r, g, b);
    painter.rect_filled(rect, 0.0, color(drawing.background));
    // A mesh keeps sharp corners from growing long fringes during path tessellation.
    let mut mesh = egui::Mesh::default();
    for triangle in drawing.triangles.iter().copied().chain(candidate) {
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
    fn set_reference(&mut self, path: PathBuf, image: DynamicImage) {
        let size = self.params.image_size;
        let image = image
            .resize(size, size, image::imageops::FilterType::Lanczos3)
            .to_rgb8();
        self.reference = Some(Reference {
            path,
            image: Arc::new(image),
        });
        self.progress = Default::default();
        self.error = None;
    }

    fn start(&mut self, ctx: egui::Context) {
        self.progress = Default::default();
        self.error = None;
        self.start_worker(ctx);
    }

    fn show_output(&self, ui: &egui::Ui, progress: &Progress) {
        let Some(drawing) = &progress.drawing else {
            return;
        };
        let (width, height) = self.reference.as_ref().unwrap().image.dimensions();
        let dimensions = egui::vec2(width as f32, height as f32);
        let available = ui.available_rect_before_wrap();
        let scale = (available.width() / dimensions.x).min(available.height() / dimensions.y);
        let rect = egui::Rect::from_center_size(available.center(), dimensions * scale);
        paint_preview(ui, rect, scale, drawing, progress.candidate);
    }
}

impl eframe::App for TriKlopsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_platform();
        let progress = self.progress.lock().unwrap().clone();
        self.show_controls(ui, &progress);
        egui::CentralPanel::default().show(ui, |ui| self.show_output(ui, &progress));
        #[cfg(not(target_arch = "wasm32"))]
        if self.worker.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(33));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ExtendedColorType, ImageEncoder, codecs::jpeg::JpegEncoder};
    use std::io::Cursor;

    #[test]
    fn decodes_jpeg_with_exif_rotation() {
        // A TIFF header and one EXIF Orientation entry (6, rotate 90° clockwise).
        let exif = b"II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut jpeg = Vec::new();
        let mut encoder = JpegEncoder::new(&mut jpeg);
        encoder.set_exif_metadata(exif.to_vec()).unwrap();
        encoder
            .encode(&[0, 60, 120, 180, 210, 255], 3, 2, ExtendedColorType::L8)
            .unwrap();

        let reader = ImageReader::new(Cursor::new(&jpeg))
            .with_guessed_format()
            .unwrap();
        let oriented = decode_reference(reader).unwrap();
        let raw = image::load_from_memory(&jpeg).unwrap();
        assert_eq!(oriented.to_luma8(), raw.rotate90().to_luma8());
    }
}
