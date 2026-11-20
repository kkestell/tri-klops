use super::TriKlopsApp;
use eframe::egui;
use std::mem::swap;
use triklops::algo::{AlgorithmParams, SearchMode};

#[derive(Debug, PartialEq)]
pub(super) struct SearchSettings {
    num_generations: usize,
    population_size: usize,
    num_selected: usize,
    mutation_rate: f64,
}

impl From<&AlgorithmParams> for SearchSettings {
    fn from(params: &AlgorithmParams) -> Self {
        Self {
            num_generations: params.num_generations,
            population_size: params.population_size,
            num_selected: params.num_selected,
            mutation_rate: params.mutation_rate,
        }
    }
}

impl SearchSettings {
    fn swap(&mut self, params: &mut AlgorithmParams) {
        swap(&mut self.num_generations, &mut params.num_generations);
        swap(&mut self.population_size, &mut params.population_size);
        swap(&mut self.num_selected, &mut params.num_selected);
        swap(&mut self.mutation_rate, &mut params.mutation_rate);
    }
}

impl TriKlopsApp {
    fn set_search_mode(&mut self, mode: SearchMode) {
        if mode == self.params.mode {
            return;
        }
        self.other_mode_settings
            .get_or_insert_with(|| SearchSettings::from(&AlgorithmParams::for_mode(mode)))
            .swap(&mut self.params);
        self.params.mode = mode;
    }

    pub(super) fn parameters(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Search mode").strong().size(14.0));
            help_button(
                ui,
                "Search mode",
                "One at a time keeps each accepted triangle in place. Whole drawing can also revisit the shapes and colors of earlier triangles.",
            );
        });
        ui.horizontal(|ui| {
            let width = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
            for (mode, label) in [
                (SearchMode::Greedy, "One at a time"),
                (SearchMode::WholeDrawing, "Whole drawing"),
            ] {
                let button = egui::Button::new(label)
                    .selected(self.params.mode == mode)
                    .min_size(egui::vec2(width, 30.0))
                    .truncate();
                if ui.add(button).clicked() {
                    self.set_search_mode(mode);
                }
            }
        });

        let whole_drawing = self.params.mode == SearchMode::WholeDrawing;
        let (label, help) = if whole_drawing {
            (
                "Triangle limit",
                "The maximum number of triangles in the drawing. The search can keep improving existing triangles after reaching this limit.",
            )
        } else {
            (
                "Triangles",
                "How many triangles to try to add. More triangles can capture finer detail and take longer to find.",
            )
        };
        slider(
            ui,
            label,
            help,
            count_slider(&mut self.params.num_triangles),
        );
        if whole_drawing {
            slider(
                ui,
                "Generations",
                "How many search steps to spend on the entire drawing. Each step can add at most one triangle, so a smaller budget can finish below the triangle limit.",
                count_slider(&mut self.params.num_generations),
            );
        }

        // The simple slider covers 0–45°; Advanced also accepts exact angles.
        let mut chunkiness = (self.params.degeneracy_threshold.unwrap_or(0.0) / 0.45).min(100.0);
        if slider(
            ui,
            "Chunkiness",
            "Higher values rule out long, skinny triangles. At 0%, there is no minimum angle.",
            egui::Slider::new(&mut chunkiness, 0.0..=100.0)
                .suffix("%")
                .integer(),
        )
        .changed()
        {
            self.params.degeneracy_threshold = (chunkiness > 0.0).then_some(chunkiness * 0.45);
        }
        slider(
            ui,
            "Grid snap",
            "Snaps triangle corners to a grid. Larger spacing makes more corners line up. At 1 px, every pixel position is available.",
            egui::Slider::new(&mut self.params.grid_spacing, 1..=32).suffix(" px"),
        );

        egui::CollapsingHeader::new("Advanced settings").show(ui, |ui| {
            self.advanced_parameters(ui);
        });
    }

    fn advanced_parameters(&mut self, ui: &mut egui::Ui) {
        if self.params.mode == SearchMode::Greedy {
            parameter_row(
                ui,
                "Generations",
                "How many search rounds to spend looking for each triangle. More rounds can find a better fit and take longer.",
                |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.params.num_generations)
                            .range(0..=usize::MAX),
                    )
                },
            );
        }
        for (label, help, value) in [
            (
                "Population size",
                "How many candidate shapes the search considers in each round. A larger population explores more shapes and takes longer.",
                &mut self.params.population_size,
            ),
            (
                "Selected",
                "How many of the highest-scoring shapes can become parents in the next round. A smaller pool focuses the search on the current leaders.",
                &mut self.params.num_selected,
            ),
        ] {
            parameter_row(ui, label, help, |ui| {
                ui.add(egui::DragValue::new(value).range(1..=usize::MAX))
            });
        }
        parameter_row(
            ui,
            "Mutation rate",
            "The chance of moving each corner when breeding a candidate shape. A higher rate tries more changes.",
            |ui| {
                ui.add(
                    egui::DragValue::new(&mut self.params.mutation_rate)
                        .range(0.0..=1.0)
                        .speed(0.01),
                )
            },
        );
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.use_seed, "Use custom seed");
            help_button(
                ui,
                "Use custom seed",
                "Use a fixed starting point for the random search. Run with the same image, settings, and seed to reproduce a drawing.",
            );
        });
        if self.use_seed {
            parameter_row(
                ui,
                "Seed",
                "Enter a nonnegative whole number, such as 42. A different seed starts a different search.",
                |ui| ui.add(egui::TextEdit::singleline(&mut self.seed).desired_width(116.0)),
            );
        }
        self.params.seed = self.use_seed.then(|| self.seed.parse().ok()).flatten();
        let mut angle = self.params.degeneracy_threshold.unwrap_or(0.0);
        if parameter_row(
            ui,
            "Minimum angle",
            "Sets Chunkiness directly in degrees. A higher minimum rules out sharper corners. Enter 0° to remove the limit.",
            |ui| ui.add(egui::DragValue::new(&mut angle).speed(0.1).suffix("°")),
        )
        .changed()
        {
            self.params.degeneracy_threshold = (angle > 0.0).then_some(angle);
        }
    }
}

fn parameter_row(
    ui: &mut egui::Ui,
    label: &str,
    help: &str,
    control: impl FnOnce(&mut egui::Ui) -> egui::Response,
) -> egui::Response {
    ui.horizontal(|ui| {
        let label = setting_label(ui, label, help);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), control)
            .inner
            .labelled_by(label.id)
    })
    .inner
}

fn count_slider(value: &mut usize) -> egui::Slider<'_> {
    // Keep the rail usable for typical counts and allow larger values in the number field.
    egui::Slider::new(value, 0..=16_384)
        .integer()
        .logarithmic(true)
        .clamping(egui::SliderClamping::Never)
}

fn slider(ui: &mut egui::Ui, label: &str, help: &str, slider: egui::Slider<'_>) -> egui::Response {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        let label = ui.horizontal(|ui| setting_label(ui, label, help)).inner;
        let width =
            ui.available_width() - ui.spacing().item_spacing.x - ui.spacing().interact_size.x;
        ui.spacing_mut().slider_width = width.max(40.0);
        ui.add(slider).labelled_by(label.id)
    })
    .inner
}

fn setting_label(ui: &mut egui::Ui, label: &str, help: &str) -> egui::Response {
    let response = ui.label(label);
    help_button(ui, label, help);
    response
}

pub(super) fn help_button(ui: &mut egui::Ui, label: &str, help: &str) {
    let (rect, button) = ui.allocate_exact_size(egui::Vec2::splat(24.0), egui::Sense::click());
    button.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::Role::Button,
            ui.is_enabled(),
            format!("Help for {label}"),
        )
    });
    let stroke = ui.style().interact(&button).fg_stroke;
    let center = rect.center();
    let painter = ui.painter();
    painter.circle_stroke(center, 6.0, stroke);
    painter.circle_filled(center + egui::vec2(0.0, -2.5), 0.8, stroke.color);
    painter.line_segment([center, center + egui::vec2(0.0, 2.8)], stroke);
    if button.has_focus() {
        painter.circle_stroke(center, 10.0, stroke);
    }

    let help_width = 220.0;
    let help_text = |ui: &mut egui::Ui| {
        ui.set_max_width(help_width);
        ui.add(egui::Label::new(help).wrap());
    };
    let popup = egui::Popup::from_toggle_button_response(&button)
        .gap(4.0)
        .width(help_width)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.strong(label);
            help_text(ui);
        });
    if popup.is_none() {
        button
            .on_hover_ui(help_text)
            .on_disabled_hover_ui(help_text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_modes_preserves_each_modes_search_settings() {
        let mut app = TriKlopsApp::default();
        app.params.num_triangles = 123;
        app.params.num_generations = 77;
        app.params.population_size = 99;
        app.params.num_selected = 9;
        app.params.mutation_rate = 0.30;
        let greedy = SearchSettings::from(&app.params);

        app.set_search_mode(SearchMode::WholeDrawing);
        assert_eq!(
            SearchSettings::from(&app.params),
            SearchSettings::from(&AlgorithmParams::for_mode(SearchMode::WholeDrawing))
        );
        assert_eq!(app.params.num_triangles, 123);

        app.params.num_generations = 321;
        app.params.population_size = 55;
        app.params.num_selected = 12;
        app.params.mutation_rate = 0.25;
        let whole = SearchSettings::from(&app.params);
        app.set_search_mode(SearchMode::WholeDrawing);
        app.set_search_mode(SearchMode::Greedy);
        assert_eq!(SearchSettings::from(&app.params), greedy);

        app.set_search_mode(SearchMode::WholeDrawing);
        assert_eq!(SearchSettings::from(&app.params), whole);
    }
}
