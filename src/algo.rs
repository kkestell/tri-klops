mod raster;
mod search;

use image::{Rgb, RgbImage};
use rand::{SeedableRng, rngs::SmallRng};
use std::{
    io,
    path::Path,
    sync::{Arc, Mutex},
};
use svg::{
    Document, Node,
    node::element::{Polygon, Rectangle},
};

pub use raster::Triangle;
use raster::{Candidate, ScoreRows};

#[derive(Clone)]
pub struct AlgorithmParams {
    pub num_triangles: usize,
    pub image_size: u32,
    pub num_generations: usize,
    pub population_size: usize,
    pub num_selected: usize,
    pub mutation_rate: f64,
    pub degeneracy_threshold: Option<f64>,
    pub seed: Option<u64>,
}

impl Default for AlgorithmParams {
    fn default() -> Self {
        Self {
            num_triangles: 512,
            image_size: 256,
            num_generations: 256,
            population_size: 128,
            num_selected: 64,
            mutation_rate: 0.1,
            degeneracy_threshold: None,
            seed: None,
        }
    }
}

#[derive(Clone, Default)]
pub struct Progress {
    pub triangle_index: usize,
    pub generation_index: usize,
    pub current_fitness: f64,
    pub should_stop: bool,
    pub canvas: Option<Arc<RgbImage>>,
    pub candidate: Option<Triangle>,
    pub revision: usize,
}

impl Progress {
    fn report_candidate(
        &mut self,
        triangle_index: usize,
        generation: usize,
        best: Option<Candidate>,
        rows: &ScoreRows,
    ) -> bool {
        self.triangle_index = triangle_index;
        self.generation_index = generation;
        let triangle = best.map(|candidate| candidate.triangle);
        if self.candidate != triangle {
            self.candidate = triangle;
            let error = rows.error - best.map_or(0, |candidate| candidate.gain);
            self.current_fitness = rows.fitness(error);
            self.revision += 1;
        }
        !self.should_stop
    }

    fn publish_canvas(&mut self, canvas: &RgbImage, fitness: f64) {
        self.canvas = Some(Arc::new(canvas.clone()));
        self.current_fitness = fitness;
        self.candidate = None;
        self.revision += 1;
    }
}

fn mean_color(reference: &RgbImage) -> [u8; 3] {
    let mut sums = [0u64; 3];
    for pixel in reference.pixels() {
        for c in 0..3 {
            sums[c] += u64::from(pixel[c]);
        }
    }
    let n = u64::from(reference.width()) * u64::from(reference.height());
    sums.map(|v| ((v + n / 2) / n) as u8)
}

fn fill([r, g, b]: [u8; 3]) -> String {
    format!("rgb({r},{g},{b})")
}

pub fn run_algorithm(
    params: &AlgorithmParams,
    reference: &RgbImage,
    output: &Path,
    progress: &Mutex<Progress>,
) -> io::Result<()> {
    let size = params.image_size;
    assert!(size > 0 && reference.dimensions() == (size, size));
    assert!(params.population_size > 0 && params.num_selected > 0);
    assert!((0.0..=1.0).contains(&params.mutation_rate));
    let mut rng = params
        .seed
        .map_or_else(rand::make_rng, SmallRng::seed_from_u64);
    let background = mean_color(reference);
    let mut canvas = RgbImage::from_pixel(size, size, Rgb(background));
    let mut document = Document::new()
        .set("width", size)
        .set("height", size)
        .set("viewBox", (0, 0, size, size))
        .set("overflow", "hidden")
        .add(
            Rectangle::new()
                .set("width", size)
                .set("height", size)
                .set("fill", fill(background)),
        );

    let mut rows = ScoreRows::new(reference, &canvas);
    progress
        .lock()
        .unwrap()
        .publish_canvas(&canvas, rows.fitness(rows.error));
    for triangle_index in 0..params.num_triangles {
        if rows.error == 0 || progress.lock().unwrap().should_stop {
            break;
        }
        let best = search::best_triangle(
            params,
            reference,
            &canvas,
            &rows,
            &mut rng,
            |generation, best| {
                progress
                    .lock()
                    .unwrap()
                    .report_candidate(triangle_index, generation, best, &rows)
            },
        );
        if let Some(best) = best {
            let triangle = best.triangle;
            rows.accept(best, &mut canvas);
            let points = triangle.vertices.map(|[x, y]| format!("{x},{y}")).join(" ");
            document.append(
                Polygon::new()
                    .set("points", points)
                    .set("fill", fill(triangle.color)),
            );
            progress
                .lock()
                .unwrap()
                .publish_canvas(&canvas, rows.fitness(rows.error));
        }
    }
    svg::save(output, &document)
}
