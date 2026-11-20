mod geometry;
mod population;
mod raster;
mod search;

use image::{Rgb, RgbImage};
use rand::{SeedableRng, rngs::SmallRng};
use std::sync::{Arc, Mutex};
#[cfg(not(target_arch = "wasm32"))]
use std::{io, path::Path};
use svg::{
    Document, Node,
    node::element::{Polygon, Rectangle},
};

pub use raster::Triangle;
use raster::{Candidate, ScoreRows, fitness};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize, serde::Deserialize))]
pub enum SearchMode {
    #[default]
    Greedy,
    WholeDrawing,
}

#[derive(Clone)]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize, serde::Deserialize))]
pub struct AlgorithmParams {
    pub mode: SearchMode,
    pub num_triangles: usize,
    pub image_size: u32,
    pub num_generations: usize,
    pub population_size: usize,
    pub num_selected: usize,
    pub mutation_rate: f64,
    pub degeneracy_threshold: Option<f64>,
    /// Distance between allowed vertex coordinates, in canvas pixels (1 is free placement).
    pub grid_spacing: u16,
    pub seed: Option<u64>,
}

impl Default for AlgorithmParams {
    fn default() -> Self {
        Self {
            mode: SearchMode::Greedy,
            num_triangles: 1024,
            image_size: 256,
            num_generations: 512,
            population_size: 128,
            num_selected: 64,
            mutation_rate: 0.1,
            degeneracy_threshold: None,
            grid_spacing: 1,
            seed: None,
        }
    }
}

#[derive(Clone, Default)]
pub struct Progress {
    pub triangle_index: usize,
    pub triangle_count: usize,
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
    ) {
        self.triangle_index = triangle_index;
        self.generation_index = generation;
        let triangle = best.map(|candidate| candidate.triangle);
        if self.candidate != triangle {
            self.candidate = triangle;
            let error = rows.error - best.map_or(0, |candidate| candidate.gain);
            self.current_fitness = fitness(error, rows.dimensions);
            self.revision += 1;
        }
    }

    fn publish_canvas(&mut self, canvas: &RgbImage, error: i64, triangle_count: usize) {
        self.triangle_count = triangle_count;
        self.canvas = Some(Arc::new(canvas.clone()));
        self.current_fitness = fitness(error, canvas.dimensions());
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

#[cfg(not(target_arch = "wasm32"))]
pub fn run_algorithm(
    params: &AlgorithmParams,
    reference: &RgbImage,
    output: &Path,
    progress: &Mutex<Progress>,
) -> io::Result<()> {
    let drawing = pollster::block_on(approximate(params, reference, progress, async |_| {}));
    svg::save(output, &drawing)
}

pub async fn approximate(
    params: &AlgorithmParams,
    reference: &RgbImage,
    progress: &Mutex<Progress>,
    mut checkpoint: impl AsyncFnMut(&Mutex<Progress>),
) -> Document {
    let size = params.image_size;
    assert!(size > 0 && reference.dimensions() == (size, size));
    assert!(params.population_size > 0 && params.num_selected > 0);
    assert!((0.0..=1.0).contains(&params.mutation_rate));
    let mut rng = params
        .seed
        .map_or_else(rand::make_rng, SmallRng::seed_from_u64);
    let background = mean_color(reference);
    let canvas = RgbImage::from_pixel(size, size, Rgb(background));
    let triangles = match params.mode {
        SearchMode::Greedy => {
            greedy(
                params,
                reference,
                canvas,
                progress,
                &mut rng,
                &mut checkpoint,
            )
            .await
        }
        SearchMode::WholeDrawing => {
            population::evolve(
                params,
                reference,
                canvas,
                progress,
                &mut rng,
                &mut checkpoint,
            )
            .await
        }
    };
    svg_document(size, background, &triangles)
}

async fn greedy(
    params: &AlgorithmParams,
    reference: &RgbImage,
    mut canvas: RgbImage,
    progress: &Mutex<Progress>,
    rng: &mut SmallRng,
    checkpoint: &mut impl AsyncFnMut(&Mutex<Progress>),
) -> Vec<Triangle> {
    let mut triangles = Vec::new();
    let mut rows = ScoreRows::new(reference, &canvas);
    progress
        .lock()
        .unwrap()
        .publish_canvas(&canvas, rows.error, 0);
    checkpoint(progress).await;
    for triangle_index in 0..params.num_triangles {
        if rows.error == 0 || progress.lock().unwrap().should_stop {
            break;
        }
        let best = search::best_triangle(
            params,
            reference,
            &canvas,
            &rows,
            rng,
            async |generation, best| {
                progress
                    .lock()
                    .unwrap()
                    .report_candidate(triangle_index, generation, best, &rows);
                checkpoint(progress).await;
                !progress.lock().unwrap().should_stop
            },
        )
        .await;
        if let Some(best) = best {
            let triangle = best.triangle;
            rows.accept(best, &mut canvas);
            triangles.push(triangle);
            progress
                .lock()
                .unwrap()
                .publish_canvas(&canvas, rows.error, triangles.len());
            checkpoint(progress).await;
        }
    }
    triangles
}

fn svg_document(size: u32, background: [u8; 3], triangles: &[Triangle]) -> Document {
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

    for triangle in triangles {
        let points = triangle.vertices.map(|[x, y]| format!("{x},{y}")).join(" ");
        document.append(
            Polygon::new()
                .set("points", points)
                .set("fill", fill(triangle.color)),
        );
    }
    document
}
