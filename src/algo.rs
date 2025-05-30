use image::{Rgb, RgbImage};
use imageproc::drawing::draw_polygon_mut;
use imageproc::point::Point;
use rand::distributions::{Distribution, Uniform};
use rand::prelude::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use svg::node::element::{Polygon, Rectangle};
use svg::Document;

#[derive(Clone)]
pub struct Triangle {
    pub vertices: [[i32; 2]; 3],
    pub color: [u8; 3],
}

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

#[derive(Clone)]
pub struct Progress {
    pub triangle_index: usize,
    pub generation_index: usize,
    pub is_running: bool,
    pub is_complete: bool,
    pub current_fitness: f64,
    pub should_stop: bool,
    pub current_generation: Vec<Triangle>,
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            triangle_index: 0,
            generation_index: 0,
            is_running: false,
            is_complete: false,
            current_fitness: f64::MIN,
            should_stop: false,
            current_generation: Vec::new(),
        }
    }
}

pub fn run_algorithm(
    params: AlgorithmParams,
    reference_image: RgbImage,
    output_path: String,
    progress: Arc<Mutex<Progress>>,
    current_canvas: Arc<Mutex<Option<RgbImage>>>,
    current_svg: Arc<Mutex<Option<Document>>>,
) {
    let seed = params.seed.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("System clock error")
            .as_secs()
    });

    let mut rng = StdRng::seed_from_u64(seed);
    let image_size = (params.image_size, params.image_size);
    let mut canvas_image = RgbImage::new(params.image_size, params.image_size);

    let mut document = Document::new()
        .set("width", params.image_size)
        .set("height", params.image_size)
        .set("viewBox", (0, 0, params.image_size, params.image_size))
        .set("overflow", "hidden")
        .add(
            Rectangle::new()
                .set("x", 0)
                .set("y", 0)
                .set("width", params.image_size)
                .set("height", params.image_size)
                .set("fill", "black"),
        );

    for triangle_index in 0..params.num_triangles {
        // Check if we should stop
        {
            let p = progress.lock().unwrap();
            if p.should_stop {
                break;
            }
        }

        {
            let mut p = progress.lock().unwrap();
            p.triangle_index = triangle_index;
            p.generation_index = 0;
        }

        let mut population = generate_initial_population(params.population_size, image_size, &mut rng);
        let mut best_triangle = None;
        let mut best_fitness = f64::MIN;

        for generation_index in 0..params.num_generations {
            // Check if we should stop
            {
                let p = progress.lock().unwrap();
                if p.should_stop {
                    break;
                }
            }

            {
                let mut p = progress.lock().unwrap();
                p.generation_index = generation_index;
            }

            let degeneracy_threshold = params.degeneracy_threshold.unwrap_or(0.0);
            let fitness_scores = evaluate_fitness_batch(
                &population,
                &canvas_image,
                &reference_image,
                degeneracy_threshold,
            );

            if let Some((triangle, &fitness)) = population
                .iter()
                .zip(fitness_scores.iter())
                .max_by(|(_, f1), (_, f2)| f1.partial_cmp(f2).unwrap())
            {
                if fitness > best_fitness {
                    best_fitness = fitness;
                    best_triangle = Some(triangle.clone());

                    let mut p = progress.lock().unwrap();
                    p.current_fitness = fitness;
                }
            }

            population = select_population(&population, &fitness_scores, params.num_selected);
            population = generate_new_population(
                &population,
                params.population_size,
                image_size,
                params.mutation_rate,
                &mut rng,
            );

            {
                let mut p = progress.lock().unwrap();
                p.current_generation = population.clone();
            }
        }

        if let Some(triangle) = best_triangle {
            draw_triangle_onto_canvas(&mut canvas_image, &triangle);
            add_triangle_to_svg(&mut document, &triangle);

            // Update shared state
            {
                let mut canvas_guard = current_canvas.lock().unwrap();
                *canvas_guard = Some(canvas_image.clone());
            }
            {
                let mut svg_guard = current_svg.lock().unwrap();
                *svg_guard = Some(document.clone());
            }
        }
    }

    // Save final result
    let _ = svg::save(&output_path, &document);

    // Mark as complete
    {
        let mut p = progress.lock().unwrap();
        p.is_running = false;
        p.is_complete = true;
        p.should_stop = false;
    }
}

fn generate_initial_population(
    pop_size: usize,
    image_size: (u32, u32),
    rng: &mut impl Rng,
) -> Vec<Triangle> {
    let x_range = Uniform::from(0..image_size.0 as i32);
    let y_range = Uniform::from(0..image_size.1 as i32);
    let color_range = Uniform::from(0..=255u8);
    let seeds: Vec<u64> = (0..pop_size).map(|_| rng.gen()).collect();

    seeds
        .into_par_iter()
        .map(|seed| {
            let mut thread_rng = StdRng::seed_from_u64(seed);
            let v1 = [
                x_range.sample(&mut thread_rng),
                y_range.sample(&mut thread_rng),
            ];
            let v2 = [
                x_range.sample(&mut thread_rng),
                y_range.sample(&mut thread_rng),
            ];
            let v3 = [
                x_range.sample(&mut thread_rng),
                y_range.sample(&mut thread_rng),
            ];
            let vertices = [v1, v2, v3];
            let color = [
                color_range.sample(&mut thread_rng),
                color_range.sample(&mut thread_rng),
                color_range.sample(&mut thread_rng),
            ];
            Triangle { vertices, color }
        })
        .collect()
}

fn mutate(
    triangle: &Triangle,
    image_size: (u32, u32),
    mutation_rate: f64,
    rng: &mut impl Rng,
) -> Triangle {
    let mut new_triangle = triangle.clone();
    if rng.gen::<f64>() < mutation_rate {
        let x_range = (image_size.0 as f64 * 0.1) as i32;
        let y_range = (image_size.1 as f64 * 0.1) as i32;

        for i in 0..3 {
            if rng.gen::<f64>() < 0.5 {
                let x = new_triangle.vertices[i][0] + rng.gen_range(-x_range..=x_range);
                let y = new_triangle.vertices[i][1] + rng.gen_range(-y_range..=y_range);
                new_triangle.vertices[i][0] = x;
                new_triangle.vertices[i][1] = y;
            }
        }
        for i in 0..3 {
            if rng.gen::<f64>() < 0.5 {
                let color_component = new_triangle.color[i] as i32 + rng.gen_range(-10..=10);
                new_triangle.color[i] = color_component.clamp(0, 255) as u8;
            }
        }
    }
    new_triangle
}

fn crossover(parent1: &Triangle, parent2: &Triangle, rng: &mut impl Rng) -> Triangle {
    let mut child_vertices = [[0i32; 2]; 3];
    for i in 0..3 {
        child_vertices[i] = if rng.gen::<f64>() < 0.5 {
            parent1.vertices[i]
        } else {
            parent2.vertices[i]
        };
    }
    let mut child_color = [0u8; 3];
    for i in 0..3 {
        child_color[i] = if rng.gen::<f64>() < 0.5 {
            parent1.color[i]
        } else {
            parent2.color[i]
        };
    }
    Triangle {
        vertices: child_vertices,
        color: child_color,
    }
}

fn generate_new_population(
    parents: &[Triangle],
    population_size: usize,
    image_size: (u32, u32),
    mutation_rate: f64,
    rng: &mut impl Rng,
) -> Vec<Triangle> {
    let seeds: Vec<u64> = (0..population_size).map(|_| rng.gen()).collect();

    seeds
        .into_par_iter()
        .map(|seed| {
            let mut thread_rng = StdRng::seed_from_u64(seed);
            let parent1 = parents.choose(&mut thread_rng).unwrap();
            let parent2 = parents.choose(&mut thread_rng).unwrap();
            let child = crossover(parent1, parent2, &mut thread_rng);
            mutate(&child, image_size, mutation_rate, &mut thread_rng)
        })
        .collect()
}

pub fn draw_triangle_onto_canvas(image: &mut RgbImage, triangle: &Triangle) {
    let mut points = triangle
        .vertices
        .iter()
        .map(|&v| Point::new(v[0], v[1]))
        .collect::<Vec<_>>();

    if points.len() > 2 && points[0] == points[points.len() - 1] {
        points.pop();
    }
    if points.len() < 3 {
        return;
    }

    let color = Rgb([triangle.color[0], triangle.color[1], triangle.color[2]]);
    draw_polygon_mut(image, &points, color);
}

fn compute_mse(image1: &RgbImage, image2: &RgbImage) -> f64 {
    assert_eq!(image1.dimensions(), image2.dimensions());

    let (width, height) = image1.dimensions();
    let total_values = (width * height * 3) as f64;

    let sum_squared_diff: f64 = image1
        .pixels()
        .zip(image2.pixels())
        .map(|(p1, p2)| {
            let rgb1 = p1.0;
            let rgb2 = p2.0;
            (rgb1[0] as f64 - rgb2[0] as f64).powi(2)
                + (rgb1[1] as f64 - rgb2[1] as f64).powi(2)
                + (rgb1[2] as f64 - rgb2[2] as f64).powi(2)
        })
        .sum();

    sum_squared_diff / total_values
}

fn is_degenerate(triangle: &Triangle, threshold: f64) -> bool {
    let a = triangle.vertices[0];
    let b = triangle.vertices[1];
    let c = triangle.vertices[2];

    let ab = ((b[0] - a[0]).pow(2) + (b[1] - a[1]).pow(2)) as f64;
    let bc = ((c[0] - b[0]).pow(2) + (c[1] - b[1]).pow(2)) as f64;
    let ca = ((a[0] - c[0]).pow(2) + (a[1] - c[1]).pow(2)) as f64;

    let angle_a = ((bc + ca - ab) / (2.0 * (bc * ca).sqrt()))
        .acos()
        .to_degrees();
    let angle_b = ((ca + ab - bc) / (2.0 * (ca * ab).sqrt()))
        .acos()
        .to_degrees();
    let angle_c = ((ab + bc - ca) / (2.0 * (ab * bc).sqrt()))
        .acos()
        .to_degrees();

    angle_a <= threshold || angle_b <= threshold || angle_c <= threshold
}

fn evaluate_fitness_batch(
    population: &[Triangle],
    canvas_image: &RgbImage,
    reference_image: &RgbImage,
    degeneracy_threshold: f64,
) -> Vec<f64> {
    population
        .par_iter()
        .map(|triangle| {
            if degeneracy_threshold > 0.0 && is_degenerate(triangle, degeneracy_threshold) {
                f64::MIN
            } else {
                let mut working_image = canvas_image.clone();
                draw_triangle_onto_canvas(&mut working_image, triangle);
                -compute_mse(&working_image, reference_image)
            }
        })
        .collect()
}

fn select_population(
    population: &[Triangle],
    fitness_scores: &[f64],
    num_selected: usize,
) -> Vec<Triangle> {
    let mut combined: Vec<_> = population.iter().zip(fitness_scores.iter()).collect();
    combined.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
    combined
        .iter()
        .take(num_selected)
        .map(|(triangle, _)| (*triangle).clone())
        .collect()
}

fn add_triangle_to_svg(document: &mut Document, triangle: &Triangle) {
    let points = triangle
        .vertices
        .iter()
        .map(|v| format!("{},{}", v[0], v[1]))
        .collect::<Vec<_>>()
        .join(" ");

    let color = format!(
        "rgb({},{},{})",
        triangle.color[0], triangle.color[1], triangle.color[2]
    );

    let polygon = Polygon::new().set("points", points).set("fill", color);

    *document = document.clone().add(polygon);
}