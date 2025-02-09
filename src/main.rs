use clap::Parser;
use image::{Rgb, RgbImage};
use imageproc::drawing::draw_polygon_mut;
use imageproc::point::Point;
use indicatif::{ProgressBar, ProgressStyle};
use rand::distributions::{Distribution, Uniform};
use rand::prelude::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;
use std::error::Error;
use std::time::SystemTime;
use svg::node::element::{Polygon, Rectangle};
use svg::Document;

#[derive(Parser)]
#[command(about = "Tri-Klops")]
struct Args {
    /// Path to the reference image
    reference_image_path: String,

    /// Output path (optional)
    #[arg(short = 'o', long = "output")]
    output_path: Option<String>,

    /// Number of triangles
    #[arg(short = 't', long = "num-triangles", default_value_t = 512)]
    num_triangles: usize,

    /// Image size (width and height)
    #[arg(short = 'i', long = "image-size", default_value_t = 256)]
    image_size: u32,

    /// Number of generations
    #[arg(short = 'g', long = "num-generations", default_value_t = 256)]
    num_generations: usize,

    /// Population size
    #[arg(short = 'p', long = "population-size", default_value_t = 128)]
    population_size: usize,

    /// Number of individuals selected per generation
    #[arg(short = 's', long = "num-selected", default_value_t = 64)]
    num_selected: usize,

    /// Mutation rate
    #[arg(short = 'm', long = "mutation-rate", default_value_t = 0.1)]
    mutation_rate: f64,

    /// Degeneracy threshold (optional)
    #[arg(short = 'd', long = "degeneracy-threshold")]
    degeneracy_threshold: Option<f64>,

    /// Save frequency
    #[arg(short = 'f', long = "save-frequency", default_value_t = 1)]
    save_frequency: usize,

    /// Seed for the random number generator (optional)
    #[arg(short = 'r', long = "seed")]
    seed: Option<u64>,

    /// Number of threads to use (optional)
    #[arg(short = 'T', long = "threads")]
    threads: Option<usize>,
}

#[derive(Clone)]
struct Triangle {
    vertices: [[i32; 2]; 3],
    color: [u8; 3],
}

fn get_seed(args_seed: Option<u64>) -> u64 {
    args_seed.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("System clock is not set correctly")
            .as_secs()
    })
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

fn draw_triangle_onto_canvas(image: &mut RgbImage, triangle: &Triangle) {
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
    assert_eq!(
        image1.dimensions(),
        image2.dimensions(),
        "Images must have the same dimensions."
    );

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

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    if let Some(num_threads) = args.threads {
        ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build_global()?;
    }

    let image_size = (args.image_size, args.image_size);
    let num_triangles = args.num_triangles;
    let num_generations = args.num_generations;
    let population_size = args.population_size;
    let num_selected = args.num_selected;
    let mutation_rate = args.mutation_rate;
    let degeneracy_threshold = args.degeneracy_threshold.unwrap_or(0.0);
    let save_frequency = args.save_frequency;

    let seed = get_seed(args.seed);
    let mut rng = StdRng::seed_from_u64(seed);

    let reference_image_path = args.reference_image_path.clone();

    let output_svg_path = if let Some(ref output_path) = args.output_path {
        output_path.clone()
    } else {
        let filename_without_extension = reference_image_path
            .rsplit('.')
            .nth(1)
            .unwrap_or(&reference_image_path);
        format!("{}.svg", filename_without_extension)
    };

    let reference_image = image::open(&reference_image_path)?
        .resize_exact(
            image_size.0,
            image_size.1,
            image::imageops::FilterType::Lanczos3,
        )
        .to_rgb8();

    let mut canvas_image = RgbImage::new(image_size.0, image_size.1);

    let mut document = Document::new()
        .set("width", image_size.0)
        .set("height", image_size.1)
        .set("viewBox", (0, 0, image_size.0, image_size.1))
        .set("overflow", "hidden");

    let mut cmd_args = vec![args.reference_image_path.clone()];
    if let Some(ref out) = args.output_path {
        cmd_args.push(format!("-o {}", out));
    }
    cmd_args.push(format!("-t {}", args.num_triangles));
    cmd_args.push(format!("-i {}", args.image_size));
    cmd_args.push(format!("-g {}", args.num_generations));
    cmd_args.push(format!("-p {}", args.population_size));
    cmd_args.push(format!("-s {}", args.num_selected));
    cmd_args.push(format!("-m {}", args.mutation_rate));
    if args.degeneracy_threshold.is_some() {
        cmd_args.push(format!("-d {}", degeneracy_threshold));
    }
    cmd_args.push(format!("-f {}", save_frequency));
    if let Some(sd) = args.seed {
        cmd_args.push(format!("-r {}", sd));
    }
    if let Some(threads) = args.threads {
        cmd_args.push(format!("-T {}", threads));
    }
    let metadata_comment = cmd_args.join(" ");
    document = document.add(svg::node::Comment::new(metadata_comment));

    document = document.add(
        Rectangle::new()
            .set("x", 0)
            .set("y", 0)
            .set("width", image_size.0)
            .set("height", image_size.1)
            .set("fill", "black"),
    );

    let triangle_progress = ProgressBar::new(num_triangles as u64).with_style(
        ProgressStyle::default_bar()
            .template("Triangles: {bar:40.cyan/blue} {pos}/{len} {eta_precise}"),
    );

    for triangle_index in 0..num_triangles {
        let generation_progress = ProgressBar::new(num_generations as u64).with_style(
            ProgressStyle::default_bar()
                .template(" Generations: {bar:40.green/blue} {pos}/{len} {eta_precise}"),
        );

        let mut population = generate_initial_population(population_size, image_size, &mut rng);

        let mut best_triangle = None;
        let mut best_fitness = f64::MIN;

        for _ in 0..num_generations {
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
                }
            }

            population = select_population(&population, &fitness_scores, num_selected);
            population = generate_new_population(
                &population,
                population_size,
                image_size,
                mutation_rate,
                &mut rng,
            );

            generation_progress.inc(1);
        }

        generation_progress.finish_and_clear();

        if let Some(triangle) = best_triangle {
            draw_triangle_onto_canvas(&mut canvas_image, &triangle);
            add_triangle_to_svg(&mut document, &triangle);
        }

        if save_frequency > 0 && triangle_index % save_frequency == 0 {
            svg::save(&output_svg_path, &document)?;
        }

        triangle_progress.inc(1);
    }

    triangle_progress.finish_and_clear();
    svg::save(&output_svg_path, &document)?;

    Ok(())
}
