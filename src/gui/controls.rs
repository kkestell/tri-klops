use super::TriKlopsApp;
#[cfg(target_arch = "wasm32")]
use super::platform::DownloadFormat;
use eframe::egui::{self, Color32};
use triklops::algo::{Progress, SearchMode};

impl TriKlopsApp {
    pub(super) fn show_controls(&mut self, ui: &mut egui::Ui, progress: &Progress) {
        #[cfg(target_arch = "wasm32")]
        {
            if ui.available_width() < 600.0 {
                self.mobile_controls(ui, progress);
                return;
            }
            self.mobile_settings_open = false;
        }

        egui::Panel::left("controls")
            .exact_size(272.0)
            .resizable(false)
            .frame(controls_frame(ui, 16))
            .show(ui, |ui| {
                style_controls(ui, false);
                egui::ScrollArea::vertical().show(ui, |ui| self.sidebar(ui, progress));
            });
    }

    fn sidebar(&mut self, ui: &mut egui::Ui, progress: &Progress) {
        ui.label(egui::RichText::new("Reference image").strong().size(14.0));
        self.image_control(ui, 34.0);
        if let Some(name) = self.reference.as_ref().and_then(|r| r.path.file_name()) {
            let name = name.to_string_lossy();
            ui.add(egui::Label::new(egui::RichText::new(name.as_ref()).weak()).truncate())
                .on_hover_text(name);
        }

        ui.separator();
        ui.add_enabled_ui(self.can_edit_settings(), |ui| self.parameters(ui));
        ui.separator();

        self.process_control(ui, "Start processing", "Stop processing", 34.0);
        #[cfg(target_arch = "wasm32")]
        if self.browser.has_output() {
            self.download_controls(ui, 34.0);
        }
        if self.worker.is_some() {
            ui.weak(self.progress_text(progress));
        }
        if progress.drawing.is_some() {
            ui.weak(format!("Fitness: {:.2}", progress.current_fitness));
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn mobile_controls(&mut self, ui: &mut egui::Ui, progress: &Progress) {
        const BUTTON_HEIGHT: f32 = 44.0;
        let mut height = 62.0;
        let running = self.worker.is_some();
        if self.browser.has_output() {
            height += BUTTON_HEIGHT + 6.0;
        }
        if running {
            height += 22.0;
        }
        if self.error.is_some() {
            height += 48.0;
        }
        egui::Panel::bottom("mobile controls")
            .exact_size(height)
            .resizable(false)
            .frame(controls_frame(ui, 8))
            .show(ui, |ui| {
                style_controls(ui, true);
                ui.columns(3, |columns| {
                    self.image_control(&mut columns[0], BUTTON_HEIGHT);
                    self.process_control(&mut columns[1], "Start", "Stop", BUTTON_HEIGHT);
                    let ui = &mut columns[2];
                    let button = control_button(ui, "Settings", BUTTON_HEIGHT, false);
                    if ui.add_enabled(self.can_edit_settings(), button).clicked() {
                        self.mobile_settings_open = true;
                    }
                });
                if self.browser.has_output() {
                    self.download_controls(ui, BUTTON_HEIGHT);
                }
                if running {
                    ui.add(
                        egui::Label::new(egui::RichText::new(self.progress_text(progress)).weak())
                            .truncate(),
                    );
                }
                if let Some(error) = &self.error {
                    egui::ScrollArea::vertical()
                        .max_height(42.0)
                        .show(ui, |ui| {
                            ui.colored_label(ui.visuals().error_fg_color, error);
                        });
                }
            });

        if self.mobile_settings_open {
            self.mobile_settings(ui.ctx());
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn mobile_settings(&mut self, ctx: &egui::Context) {
        let available = ctx.content_rect().size();
        let response = egui::Modal::new(egui::Id::unique("mobile settings")).show(ctx, |ui| {
            style_controls(ui, true);
            ui.set_width((available.x - 40.0).min(400.0));
            ui.heading("Settings");
            egui::ScrollArea::vertical()
                .max_height((available.y - 112.0).max(44.0))
                .show(ui, |ui| self.parameters(ui));
            ui.add(control_button(ui, "Done", 44.0, true)).clicked()
        });
        if response.inner || response.should_close() {
            self.mobile_settings_open = false;
        }
    }

    fn is_loading(&self) -> bool {
        #[cfg(target_arch = "wasm32")]
        return self.browser.is_loading();
        #[cfg(not(target_arch = "wasm32"))]
        false
    }

    fn can_edit_settings(&self) -> bool {
        self.worker.is_none() && !self.is_loading() && self.reference.is_some()
    }

    fn image_control(&mut self, ui: &mut egui::Ui, height: f32) {
        let loading = self.is_loading();
        let text = match (loading, self.reference.is_some()) {
            (true, _) => "Loading…",
            (false, true) => "Replace image",
            (false, false) => "Choose image",
        };
        let enabled = self.worker.is_none() && !loading;
        let button = control_button(ui, text, height, enabled && self.reference.is_none());
        if ui.add_enabled(enabled, button).clicked() {
            self.load_reference(ui.ctx());
        }
    }

    fn process_control(&mut self, ui: &mut egui::Ui, start: &str, stop: &str, height: f32) {
        if self.worker.is_some() {
            if ui.add(control_button(ui, stop, height, false)).clicked() {
                self.stop_worker();
            }
        } else {
            let valid = self.can_edit_settings() && (!self.use_seed || self.params.seed.is_some());
            let button = control_button(ui, start, height, valid);
            if ui.add_enabled(valid, button).clicked() {
                self.start(ui.ctx().clone());
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn download_controls(&mut self, ui: &mut egui::Ui, height: f32) {
        ui.columns(2, |columns| {
            for (ui, (format, label)) in columns.iter_mut().zip([
                (DownloadFormat::Svg, "Download SVG"),
                (DownloadFormat::Jpg, "Download JPG"),
            ]) {
                if ui.add(control_button(ui, label, height, false)).clicked() {
                    self.download_output(format);
                }
            }
        });
    }

    fn progress_text(&self, progress: &Progress) -> String {
        match self.params.mode {
            SearchMode::Greedy => format!(
                "Triangle {} / {} · Generation {}",
                progress.triangle_index + 1,
                self.params.num_triangles,
                progress.generation_index + 1
            ),
            SearchMode::WholeDrawing => format!(
                "Generation {} / {} · {} triangles",
                (progress.generation_index + 1).min(self.params.num_generations),
                self.params.num_generations,
                progress.triangle_count
            ),
        }
    }
}

fn controls_frame(ui: &egui::Ui, margin: i8) -> egui::Frame {
    egui::Frame::side_top_panel(ui.style())
        .inner_margin(margin)
        .fill(if ui.visuals().dark_mode {
            Color32::from_rgb(34, 38, 44)
        } else {
            Color32::from_rgb(238, 242, 246)
        })
}

fn style_controls(ui: &mut egui::Ui, narrow: bool) {
    let style = ui.style_mut();
    style.spacing.item_spacing = egui::vec2(6.0, if narrow { 6.0 } else { 8.0 });
    style.spacing.button_padding = egui::vec2(if narrow { 6.0 } else { 8.0 }, 4.0);
    style.spacing.interact_size = egui::vec2(56.0, 24.0);
    style.spacing.slider_rail_height = 4.0;
    style.spacing.indent = 12.0;
    let visuals = &mut style.visuals;
    visuals.indent_has_left_vline = false;
    visuals.slider_trailing_fill = true;
    visuals.handle_shape = egui::style::HandleShape::Circle;
    let (selected, accent, border, muted_text) = if visuals.dark_mode {
        (
            Color32::from_rgb(48, 71, 99),
            Color32::from_rgb(168, 202, 255),
            Color32::from_rgb(60, 67, 78),
            Color32::from_rgb(168, 176, 186),
        )
    } else {
        (
            Color32::from_rgb(219, 233, 252),
            Color32::from_rgb(25, 71, 135),
            Color32::from_rgb(203, 213, 224),
            Color32::from_rgb(86, 100, 118),
        )
    };
    visuals.weak_text_color = Some(muted_text);
    visuals.selection.bg_fill = selected;
    visuals.selection.stroke = egui::Stroke::new(1.0, accent);
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, border);
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
    ] {
        widget.corner_radius = egui::CornerRadius::same(6);
        widget.expansion = 0.0;
    }
}

fn control_button(ui: &egui::Ui, label: &str, height: f32, primary: bool) -> egui::Button<'static> {
    let mut text = egui::RichText::new(label);
    if primary {
        text = text.color(Color32::WHITE);
    }
    let button = egui::Button::new((egui::Atom::grow(), text, egui::Atom::grow()))
        .gap(0.0)
        .min_size(egui::vec2(ui.available_width(), height))
        .truncate();
    if primary {
        button.fill(Color32::from_rgb(36, 89, 164))
    } else {
        button
    }
}
