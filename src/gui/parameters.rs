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
                    "Add triangles and refine earlier layers.",
                ),
            ] {
                let button = egui::Button::new(label)
                    .selected(self.params.mode == mode)
                    .min_size(egui::vec2(width, 30.0))
                    .truncate();
                if ui.add(button).on_hover_text(hint).clicked() {
                    self.set_search_mode(mode);
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
