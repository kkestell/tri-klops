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
    /// Length of the longest side of the processing canvas, in pixels.
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
            num_triangles: 2048,
            image_size: 256,
            num_generations: 1024,
            population_size: 128,
            num_selected: 64,
            mutation_rate: 0.1,
            degeneracy_threshold: Some(9.0),
            grid_spacing: 1,
            seed: None,
        }
    }
}

#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize, serde::Deserialize))]
pub struct Drawing {
    pub background: [u8; 3],
    pub triangles: Vec<Triangle>,
}

#[derive(Clone, Default)]
pub struct Progress {
    pub triangle_index: usize,
    pub triangle_count: usize,
    pub generation_index: usize,
    pub current_fitness: f64,
    pub should_stop: bool,
    pub canvas: Option<Arc<RgbImage>>,
    pub drawing: Option<Arc<Drawing>>,
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

    fn publish_drawing(
        &mut self,
        canvas: &RgbImage,
        error: i64,
        background: [u8; 3],
        triangles: &[Triangle],
    ) {
        self.triangle_count = triangles.len();
        self.canvas = Some(Arc::new(canvas.clone()));
        self.drawing = Some(Arc::new(Drawing {
            background,
            triangles: triangles.to_vec(),
        }));
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
    let (width, height) = reference.dimensions();
    assert!(width > 0 && height > 0 && width.max(height) == params.image_size);
    assert!(params.population_size > 0 && params.num_selected > 0);
    assert!((0.0..=1.0).contains(&params.mutation_rate));
    let mut rng = params
        .seed
        .map_or_else(rand::make_rng, SmallRng::seed_from_u64);
    let background = mean_color(reference);
    let canvas = RgbImage::from_pixel(width, height, Rgb(background));
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
    svg_document((width, height), background, &triangles)
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
    let background = canvas.get_pixel(0, 0).0;
    progress
        .lock()
        .unwrap()
        .publish_drawing(&canvas, rows.error, background, &triangles);
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
                .publish_drawing(&canvas, rows.error, background, &triangles);
            checkpoint(progress).await;
        }
    }
    triangles
}

fn svg_document(
    (width, height): (u32, u32),
    background: [u8; 3],
    triangles: &[Triangle],
) -> Document {
    let mut document = Document::new()
        .set("width", width)
        .set("height", height)
        .set("viewBox", (0, 0, width, height))
        .set("overflow", "hidden")
        .add(
            Rectangle::new()
                .set("width", width)
                .set("height", height)
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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn rectangular_drawings_match_the_scoring_canvas_and_svg() {
        for (width, height) in [(32, 12), (12, 32)] {
            let reference = RgbImage::from_fn(width, height, |x, y| {
                Rgb([(x * 7) as u8, (y * 7) as u8, 128])
            });
            for mode in [SearchMode::Greedy, SearchMode::WholeDrawing] {
                let params = AlgorithmParams {
                    mode,
                    image_size: 32,
                    num_triangles: 8,
                    num_generations: 8,
                    population_size: 12,
                    num_selected: 4,
                    seed: Some(42),
                    ..Default::default()
                };
                let progress = Mutex::new(Progress::default());
                let svg =
                    pollster::block_on(approximate(&params, &reference, &progress, async |_| {}))
                        .to_string();
                let progress = progress.into_inner().unwrap();
                let drawing = progress.drawing.unwrap();
                assert!(!drawing.triangles.is_empty());
                assert!(progress.candidate.is_none());
                assert!(svg.contains(&format!("viewBox=\"0 0 {width} {height}\"")));
                assert_eq!(svg.matches("<polygon").count(), drawing.triangles.len());

                let mut painted = RgbImage::from_pixel(width, height, Rgb(drawing.background));
                for triangle in &drawing.triangles {
                    triangle.paint(&mut painted);
                }
                assert_eq!(progress.canvas.as_deref(), Some(&painted));
            }
        }
    }
}
