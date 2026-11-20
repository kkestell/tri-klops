use eframe::egui;
use image::RgbImage;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};
use triklops::algo::{AlgorithmParams, Progress, run_algorithm};

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
    worker: Option<JoinHandle<std::io::Result<()>>>,
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
    fn load_reference(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif"])
            .pick_file()
        else {
            return;
        };
        match image::open(&path) {
            Ok(image) => {
                let size = self.params.image_size;
                let image = image
                    .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
                    .to_rgb8();
                let texture =
                    ctx.load_texture("reference", color_image(&image), Default::default());
                self.reference = Some(Reference {
                    path,
                    image: Arc::new(image),
                    texture,
                });
                self.reset_preview();
                self.error = None;
            }
            Err(e) => self.error = Some(format!("Could not load {}: {e}", path.display())),
        }
    }

    fn reset_preview(&mut self) {
        self.progress = Default::default();
        self.preview = None;
        self.preview_revision = 0;
    }

    fn start(&mut self, ctx: egui::Context) {
        self.reset_preview();
        let reference = self.reference.as_ref().unwrap();
        let image = reference.image.clone();
        let output = reference.path.with_extension("svg");
        let progress = self.progress.clone();
        let params = self.params.clone();
        self.error = None;
        self.worker = Some(thread::spawn(move || {
            let result = run_algorithm(&params, &image, &output, &progress);
            ctx.request_repaint();
            result
        }));
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

    fn parameters(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Triangles");
            ui.add(egui::DragValue::new(&mut self.params.num_triangles).range(0..=usize::MAX));
        });
        // The simple slider covers 0–45°; Advanced also accepts exact angles.
        let mut chunkiness = (self.params.degeneracy_threshold.unwrap_or(0.0) / 0.45).min(100.0);
        if ui
            .add(
                egui::Slider::new(&mut chunkiness, 0.0..=100.0)
                    .text("Chunkiness")
                    .suffix("%")
                    .integer(),
            )
            .on_hover_text("Higher values rule out long, skinny triangles.")
            .changed()
        {
            self.params.degeneracy_threshold = (chunkiness > 0.0).then_some(chunkiness * 0.45);
        }
        ui.add(
            egui::Slider::new(&mut self.params.grid_spacing, 1..=32)
                .text("Grid snap")
                .suffix(" px"),
        )
        .on_hover_text(
            "Space triangle corners farther apart on a square grid. 1 px allows every position.",
        );
        ui.collapsing("Advanced", |ui| self.advanced_parameters(ui));
    }

    fn advanced_parameters(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("advanced-params").show(ui, |ui| {
            for (label, value, minimum) in [
                ("Generations", &mut self.params.num_generations, 0),
                ("Population size", &mut self.params.population_size, 1),
                ("Selected", &mut self.params.num_selected, 1),
            ] {
                ui.label(label);
                ui.add(egui::DragValue::new(value).range(minimum..=usize::MAX));
                ui.end_row();
            }
            ui.label("Mutation rate");
            ui.add(
                egui::DragValue::new(&mut self.params.mutation_rate)
                    .range(0.0..=1.0)
                    .speed(0.01),
            );
            ui.end_row();
            ui.label("Use custom seed");
            ui.checkbox(&mut self.use_seed, "");
            ui.end_row();
            if self.use_seed {
                ui.label("Seed");
                ui.add(egui::TextEdit::singleline(&mut self.seed).desired_width(80.0));
                ui.end_row();
            }
            self.params.seed = self.use_seed.then(|| self.seed.parse().ok()).flatten();
            ui.label("Minimum angle");
            let mut angle = self.params.degeneracy_threshold.unwrap_or(0.0);
            if ui
                .add(egui::DragValue::new(&mut angle).speed(0.1).suffix("°"))
                .changed()
            {
                self.params.degeneracy_threshold = (angle > 0.0).then_some(angle);
            }
            ui.end_row();
        });
    }

    fn controls(&mut self, ui: &mut egui::Ui, p: &Progress) {
        let running = self.worker.is_some();
        if ui
            .add_enabled(!running, egui::Button::new("Load Reference Image…"))
            .clicked()
        {
            self.load_reference(ui.ctx());
        }
        ui.separator();
        ui.add_enabled_ui(!running && self.reference.is_some(), |ui| {
            self.parameters(ui)
        });
        ui.separator();
        if running {
            if ui.button("Stop Processing").clicked() {
                self.progress.lock().unwrap().should_stop = true;
            }
            ui.label(format!(
                "Triangle {} / {} · Generation {}",
                p.triangle_index + 1,
                self.params.num_triangles,
                p.generation_index + 1
            ));
        } else {
            let valid = self.reference.is_some() && (!self.use_seed || self.params.seed.is_some());
            if ui
                .add_enabled(valid, egui::Button::new("Start Processing"))
                .clicked()
            {
                self.start(ui.ctx().clone());
            }
        }
        if p.canvas.is_some() {
            ui.label(format!("Fitness: {:.2}", p.current_fitness));
        }
        if let Some(error) = &self.error {
            ui.label(error);
        }
    }
}

impl eframe::App for TriKlopsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.worker.as_ref().is_some_and(|w| w.is_finished()) {
            self.error = match self.worker.take().unwrap().join() {
                Ok(Ok(())) => None,
                Ok(Err(err)) => Some(format!("Could not save SVG: {err}")),
                Err(_) => Some("Image generation failed.".into()),
            };
        }
        let progress = self.progress.lock().unwrap().clone();
        self.update_preview(ui.ctx(), &progress);
        egui::Panel::left("controls")
            .exact_size(230.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.controls(ui, &progress));
            });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                for texture in [
                    self.reference.as_ref().map(|r| &r.texture),
                    self.preview.as_ref(),
                ] {
                    if let Some(texture) = texture {
                        ui.image(texture);
                    } else {
                        let size = egui::Vec2::splat(self.params.image_size as f32);
                        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                        ui.painter().rect_filled(rect, 0.0, egui::Color32::BLACK);
                    }
                }
            });
        });
        if self.worker.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(33));
        }
    }
}
