use super::TriKlopsApp;
use eframe::egui::{self, Color32};
use triklops::algo::{Progress, SearchMode};

impl TriKlopsApp {
    pub(super) fn show_controls(&mut self, ui: &mut egui::Ui, progress: &Progress) {
        let narrow = cfg!(target_arch = "wasm32") && ui.available_width() < 600.0;
        let frame = egui::Frame::side_top_panel(ui.style())
            .inner_margin(if narrow { 12 } else { 16 })
            .fill(if ui.visuals().dark_mode {
                Color32::from_rgb(34, 38, 44)
            } else {
                Color32::from_rgb(238, 242, 246)
            });
        let panel = if narrow {
            egui::Panel::top("controls").exact_size((ui.available_height() * 0.55).min(400.0))
        } else {
            egui::Panel::left("controls").exact_size(272.0)
        };
        panel.resizable(false).frame(frame).show(ui, |ui| {
            style_controls(ui, narrow);
            egui::ScrollArea::vertical().show(ui, |ui| self.controls(ui, progress));
        });
    }

    fn controls(&mut self, ui: &mut egui::Ui, progress: &Progress) {
        let running = self.worker.is_some();
        #[cfg(target_arch = "wasm32")]
        let loading = self.browser.is_loading();
        #[cfg(not(target_arch = "wasm32"))]
        let loading = false;

        ui.label(egui::RichText::new("Reference image").strong().size(14.0));
        let image_action = match (loading, &self.reference) {
            (true, _) => "Loading image…",
            (false, Some(_)) => "Replace image…",
            (false, None) => "Choose image…",
        };
        let image_button = if self.reference.is_none() && !loading {
            primary_button(ui, image_action)
        } else {
            sidebar_button(ui, image_action)
        };
        if ui.add_enabled(!running && !loading, image_button).clicked() {
            self.load_reference(ui.ctx());
        }
        if let Some(name) = self.reference.as_ref().and_then(|r| r.path.file_name()) {
            let name = name.to_string_lossy();
            ui.add(egui::Label::new(egui::RichText::new(name.as_ref()).weak()).truncate())
                .on_hover_text(name);
        }

        ui.separator();
        ui.add_enabled_ui(!running && !loading && self.reference.is_some(), |ui| {
            self.parameters(ui)
        });
        ui.separator();

        if running {
            if ui.add(sidebar_button(ui, "Stop processing")).clicked() {
                self.stop_worker();
            }
            match self.params.mode {
                SearchMode::Greedy => ui.weak(format!(
                    "Triangle {} / {} · Generation {}",
                    progress.triangle_index + 1,
                    self.params.num_triangles,
                    progress.generation_index + 1
                )),
                SearchMode::WholeDrawing => ui.weak(format!(
                    "Generation {} / {} · {} triangles",
                    (progress.generation_index + 1).min(self.params.num_generations),
                    self.params.num_generations,
                    progress.triangle_count
                )),
            };
        } else {
            let valid = !loading
                && self.reference.is_some()
                && (!self.use_seed || self.params.seed.is_some());
            let start_button = if valid {
                primary_button(ui, "Start processing")
            } else {
                sidebar_button(ui, "Start processing")
            };
            if ui.add_enabled(valid, start_button).clicked() {
                self.start(ui.ctx().clone());
            }
            #[cfg(target_arch = "wasm32")]
            self.download_control(ui);
        }
        if progress.drawing.is_some() {
            ui.weak(format!("Fitness: {:.2}", progress.current_fitness));
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }

    fn parameters(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Search mode").strong().size(14.0));
        ui.horizontal(|ui| {
            let width = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
            for (mode, label, hint) in [
                (
                    SearchMode::Greedy,
                    "One at a time",
                    "Add one triangle at a time.",
                ),
                (
                    SearchMode::WholeDrawing,
                    "Whole drawing",
                    "Evolve complete drawings within the triangle limit.",
                ),
            ] {
                let button = egui::Button::new(label)
                    .selected(self.params.mode == mode)
                    .min_size(egui::vec2(width, 30.0))
                    .truncate();
                if ui.add(button).on_hover_text(hint).clicked() {
                    self.params.mode = mode;
                }
            }
        });

        parameter_row(
            ui,
            match self.params.mode {
                SearchMode::Greedy => "Triangles",
                SearchMode::WholeDrawing => "Triangle limit",
            },
            |ui| ui.add(egui::DragValue::new(&mut self.params.num_triangles).range(0..=usize::MAX)),
        );

        // The simple slider covers 0–45°; Advanced also accepts exact angles.
        let mut chunkiness = (self.params.degeneracy_threshold.unwrap_or(0.0) / 0.45).min(100.0);
        if slider(
            ui,
            "Chunkiness",
            egui::Slider::new(&mut chunkiness, 0.0..=100.0)
                .suffix("%")
                .integer(),
        )
        .on_hover_text("Higher values rule out long, skinny triangles.")
        .changed()
        {
            self.params.degeneracy_threshold = (chunkiness > 0.0).then_some(chunkiness * 0.45);
        }
        slider(
            ui,
            "Grid snap",
            egui::Slider::new(&mut self.params.grid_spacing, 1..=32).suffix(" px"),
        )
        .on_hover_text(
            "Space triangle corners farther apart on a square grid. 1 px allows every position.",
        );

        egui::CollapsingHeader::new("Advanced settings").show(ui, |ui| {
            self.advanced_parameters(ui);
        });
    }

    fn advanced_parameters(&mut self, ui: &mut egui::Ui) {
        for (label, value, minimum) in [
            ("Generations", &mut self.params.num_generations, 0),
            ("Population size", &mut self.params.population_size, 1),
            ("Selected", &mut self.params.num_selected, 1),
        ] {
            parameter_row(ui, label, |ui| {
                ui.add(egui::DragValue::new(value).range(minimum..=usize::MAX))
            });
        }
        parameter_row(ui, "Mutation rate", |ui| {
            ui.add(
                egui::DragValue::new(&mut self.params.mutation_rate)
                    .range(0.0..=1.0)
                    .speed(0.01),
            )
        });
        ui.checkbox(&mut self.use_seed, "Use custom seed");
        if self.use_seed {
            parameter_row(ui, "Seed", |ui| {
                ui.add(egui::TextEdit::singleline(&mut self.seed).desired_width(116.0))
            });
        }
        self.params.seed = self.use_seed.then(|| self.seed.parse().ok()).flatten();
        let mut angle = self.params.degeneracy_threshold.unwrap_or(0.0);
        if parameter_row(ui, "Minimum angle", |ui| {
            ui.add(egui::DragValue::new(&mut angle).speed(0.1).suffix("°"))
        })
        .changed()
        {
            self.params.degeneracy_threshold = (angle > 0.0).then_some(angle);
        }
    }
}

fn style_controls(ui: &mut egui::Ui, narrow: bool) {
    let style = ui.style_mut();
    style.spacing.item_spacing = egui::vec2(6.0, if narrow { 4.0 } else { 8.0 });
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
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

pub(super) fn sidebar_button(
    ui: &egui::Ui,
    label: impl Into<egui::WidgetText>,
) -> egui::Button<'static> {
    egui::Button::new((egui::Atom::grow(), label.into(), egui::Atom::grow()))
        .min_size(egui::vec2(ui.available_width(), 34.0))
}

fn primary_button(ui: &egui::Ui, label: &str) -> egui::Button<'static> {
    sidebar_button(ui, egui::RichText::new(label).color(Color32::WHITE))
        .fill(Color32::from_rgb(36, 89, 164))
}

fn parameter_row(
    ui: &mut egui::Ui,
    label: &str,
    control: impl FnOnce(&mut egui::Ui) -> egui::Response,
) -> egui::Response {
    ui.horizontal(|ui| {
        let label = ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), control)
            .inner
            .labelled_by(label.id)
    })
    .inner
}

fn slider(ui: &mut egui::Ui, label: &str, slider: egui::Slider<'_>) -> egui::Response {
    let width = ui.available_width() - ui.spacing().item_spacing.x - ui.spacing().interact_size.x;
    ui.spacing_mut().slider_width = width.max(40.0);
    let label = ui.label(label);
    ui.add(slider).labelled_by(label.id)
}
