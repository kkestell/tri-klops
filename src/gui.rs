use crate::algo::{draw_triangle_onto_canvas, run_algorithm, AlgorithmParams, Progress};
use eframe::egui;
use image::RgbImage;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use svg::Document;

pub struct TriKlopsApp {
    params: AlgorithmParams,
    reference_image_path: String,
    progress: Arc<Mutex<Progress>>,
    reference_image: Option<RgbImage>,
    current_canvas: Arc<Mutex<Option<RgbImage>>>,
    current_svg: Arc<Mutex<Option<Document>>>,
    use_custom_seed: bool,
    custom_seed: String,
    use_degeneracy_threshold: bool,
    degeneracy_threshold_value: f32,
}

impl Default for TriKlopsApp {
    fn default() -> Self {
        Self {
            params: AlgorithmParams::default(),
            reference_image_path: String::new(),
            progress: Arc::new(Mutex::new(Progress::default())),
            reference_image: None,
            current_canvas: Arc::new(Mutex::new(None)),
            current_svg: Arc::new(Mutex::new(None)),
            use_custom_seed: false,
            custom_seed: String::new(),
            use_degeneracy_threshold: false,
            degeneracy_threshold_value: 1.0,
        }
    }
}

impl TriKlopsApp {
    fn create_black_square_texture(&self, ctx: &egui::Context, texture_name: &str) -> egui::TextureHandle {
        let size = [256, 256];
        let pixels: Vec<egui::Color32> = vec![egui::Color32::BLACK; 256 * 256];
        let color_image = egui::ColorImage { size, pixels };
        ctx.load_texture(texture_name, color_image, egui::TextureOptions::default())
    }

    fn render_generation_preview(&self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let progress = self.progress.lock().unwrap();

        if progress.is_running && !progress.current_generation.is_empty() {
            // Create a full-sized image with all triangles from current generation
            let mut generation_image = RgbImage::new(self.params.image_size, self.params.image_size);

            // Start with current canvas as base
            if let Ok(canvas_guard) = self.current_canvas.try_lock() {
                if let Some(ref canvas) = *canvas_guard {
                    generation_image = canvas.clone();
                }
            }

            // Draw all triangles from current generation on top
            for triangle in &progress.current_generation {
                draw_triangle_onto_canvas(&mut generation_image, triangle);
            }

            // Convert to egui texture
            let size = [generation_image.width() as usize, generation_image.height() as usize];
            let pixels: Vec<egui::Color32> = generation_image
                .pixels()
                .map(|p| egui::Color32::from_rgb(p.0[0], p.0[1], p.0[2]))
                .collect();

            let color_image = egui::ColorImage { size, pixels };
            let texture = ctx.load_texture(
                "generation_preview",
                color_image,
                egui::TextureOptions::default(),
            );
            ui.image(&texture);
        } else {
            let texture = self.create_black_square_texture(ctx, "generation_black");
            ui.image(&texture);
        }
    }

    fn get_output_path(&self) -> String {
        if self.reference_image_path.is_empty() {
            return "output.svg".to_string();
        }

        let path = Path::new(&self.reference_image_path);
        match path.with_extension("svg").to_str() {
            Some(svg_path) => svg_path.to_string(),
            None => "output.svg".to_string(),
        }
    }

    fn load_reference_image(&mut self) {
        if !self.reference_image_path.is_empty() {
            if let Ok(img) = image::open(&self.reference_image_path) {
                self.reference_image = Some(
                    img.resize_exact(
                        self.params.image_size,
                        self.params.image_size,
                        image::imageops::FilterType::Lanczos3,
                    )
                        .to_rgb8(),
                );
            }
        }
    }

    fn start_algorithm(&mut self, ctx: &egui::Context) {
        if self.reference_image.is_none() {
            // This check is important, though the button should also be disabled.
            eprintln!("Attempted to start algorithm without a reference image.");
            return;
        }

        let params = self.params.clone();
        let reference_image = self.reference_image.clone().unwrap(); // Safe due to check above
        let output_path = self.get_output_path();
        let progress_arc = Arc::clone(&self.progress);
        let current_canvas_arc = Arc::clone(&self.current_canvas);
        let current_svg_arc = Arc::clone(&self.current_svg);
        let ctx_clone = ctx.clone();

        // Reset progress
        {
            let mut p = progress_arc.lock().unwrap();
            *p = Progress {
                is_running: true,
                should_stop: false,
                ..Progress::default()
            };
        }

        // Initialize canvas and SVG
        {
            let mut canvas = current_canvas_arc.lock().unwrap();
            *canvas = Some(RgbImage::new(params.image_size, params.image_size));
        }

        {
            let mut svg = current_svg_arc.lock().unwrap();
            *svg = Some(
                Document::new()
                    .set("width", params.image_size)
                    .set("height", params.image_size)
                    .set("viewBox", (0, 0, params.image_size, params.image_size))
                    .set("overflow", "hidden")
                    .add(
                        svg::node::element::Rectangle::new()
                            .set("x", 0)
                            .set("y", 0)
                            .set("width", params.image_size)
                            .set("height", params.image_size)
                            .set("fill", "black"),
                    ),
            );
        }

        thread::spawn(move || {
            run_algorithm(
                params,
                reference_image,
                output_path,
                progress_arc,
                current_canvas_arc,
                current_svg_arc,
            );
            ctx_clone.request_repaint();
        });
    }

    fn stop_algorithm(&mut self) {
        let mut p = self.progress.lock().unwrap();
        p.should_stop = true;
    }
}

impl eframe::App for TriKlopsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update params based on UI state
        if self.use_custom_seed {
            if let Ok(seed) = self.custom_seed.parse::<u64>() {
                self.params.seed = Some(seed);
            }
        } else {
            self.params.seed = None;
        }

        if self.use_degeneracy_threshold {
            self.params.degeneracy_threshold = Some(self.degeneracy_threshold_value as f64);
        } else {
            self.params.degeneracy_threshold = None;
        }

        let progress_data = self.progress.lock().unwrap().clone();
        let has_reference_image = self.reference_image.is_some();

        egui::SidePanel::left("controls")
            .exact_width(230.0)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.add_space(16.0);

                    // Load Reference Image button (conditionally enabled, centered)
                    ui.add_enabled_ui(!progress_data.is_running, |ui| {
                        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                            let load_button = egui::Button::new("Load Reference Image...");
                            if ui.add(load_button).clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif"])
                                    .pick_file()
                                {
                                    self.reference_image_path = path.display().to_string();
                                    self.load_reference_image();
                                }
                            }
                        });
                    });


                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(12.0);

                    // Algorithm parameters (conditionally enabled)
                    ui.add_enabled_ui(has_reference_image && !progress_data.is_running, |ui| {
                        egui::Grid::new("params_grid")
                            .spacing(egui::vec2(8.0, 8.0))
                            .show(ui, |ui| {
                                ui.label("Triangles:");
                                ui.add(egui::DragValue::new(&mut self.params.num_triangles).speed(1.0));
                                ui.end_row();

                                ui.label("Generations:");
                                ui.add(
                                    egui::DragValue::new(&mut self.params.num_generations).speed(1.0),
                                );
                                ui.end_row();

                                ui.label("Population Size:");
                                ui.add(
                                    egui::DragValue::new(&mut self.params.population_size).speed(1.0),
                                );
                                ui.end_row();

                                ui.label("Selected:");
                                ui.add(egui::DragValue::new(&mut self.params.num_selected).speed(1.0));
                                ui.end_row();

                                ui.label("Mutation Rate:");
                                ui.add(
                                    egui::DragValue::new(&mut self.params.mutation_rate).speed(0.01),
                                );
                                ui.end_row();

                                ui.label("Use Custom Seed:");
                                ui.checkbox(&mut self.use_custom_seed, "");
                                ui.end_row();

                                if self.use_custom_seed {
                                    ui.label("Seed:");
                                    ui.text_edit_singleline(&mut self.custom_seed);
                                    ui.end_row();
                                }

                                ui.label("Use Degeneracy Threshold:");
                                ui.checkbox(&mut self.use_degeneracy_threshold, "");
                                ui.end_row();

                                if self.use_degeneracy_threshold {
                                    ui.label("Threshold:");
                                    ui.add(
                                        egui::DragValue::new(&mut self.degeneracy_threshold_value)
                                            .speed(0.1),
                                    );
                                    ui.end_row();
                                }
                            });
                    });

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(12.0);

                    // Control buttons (Start/Stop, centered)
                    if progress_data.is_running {
                        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                            let stop_button = egui::Button::new("Stop Processing");
                            if ui.add(stop_button).clicked() {
                                self.stop_algorithm();
                            }
                            ui.add_space(8.0);
                            ui.label(format!(
                                "Triangle: {}/{}, Fitness: {:.2}",
                                progress_data.triangle_index + 1,
                                self.params.num_triangles,
                                progress_data.current_fitness
                            ));
                        });
                    } else {
                        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                            ui.scope(|ui| {
                                ui.set_enabled(has_reference_image);
                                let start_button = egui::Button::new("Start Processing");
                                if ui.add(start_button).clicked() {
                                    if has_reference_image {
                                        self.start_algorithm(ctx);
                                    }
                                }
                            });
                        });
                    }
                    ui.add_space(8.0);
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Reference image
                ui.vertical(|ui| {
                    if let Some(ref img) = self.reference_image {
                        let size = [img.width() as usize, img.height() as usize];
                        let pixels: Vec<egui::Color32> = img
                            .pixels()
                            .map(|p| egui::Color32::from_rgb(p.0[0], p.0[1], p.0[2]))
                            .collect();

                        let color_image = egui::ColorImage { size, pixels };
                        let texture = ctx.load_texture(
                            "reference",
                            color_image,
                            egui::TextureOptions::default(),
                        );
                        ui.image(&texture);
                    } else {
                        let texture = self.create_black_square_texture(ctx, "reference_black");
                        ui.image(&texture);
                    }
                });

                ui.separator();

                // Generation preview
                ui.vertical(|ui| {
                    self.render_generation_preview(ui, ctx);
                });
            });
        });

        if progress_data.is_running {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
}
