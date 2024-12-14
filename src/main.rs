use clap::Parser;
use image::{Rgba, RgbaImage};
use imageproc::drawing::draw_polygon_mut;
use imageproc::point::Point;
use indicatif::{ProgressBar, ProgressStyle};
use rand::distributions::{Distribution, Uniform};
use rand::prelude::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;
use std::error::Error;
use std::time::SystemTime;
use svg::node::element::{Polygon, Rectangle};
use svg::Document;

/// Tri-Klops
#[derive(Parser)]
#[command(about = "Tri-Klops")]
struct Args {
    /// Path to the reference image
    reference_image_path: String,

    /// Image size (width and height)
    #[arg(long, default_value_t = 256)]
    image_size: u32,

    /// Number of triangles
    #[arg(long, default_value_t = 512)]
    num_triangles: usize,

    /// Number of generations
    #[arg(long, default_value_t = 512)]
    num_generations: usize,

    /// Population size
    #[arg(long, default_value_t = 512)]
    population_size: usize,

    /// Number of individuals selected per generation
    #[arg(long, default_value_t = 256)]
    num_selected: usize,

    /// Mutation rate
    #[arg(long, default_value_t = 0.1)]
    mutation_rate: f64,

    /// Seed for the random number generator (optional)
    #[arg(long)]
    seed: Option<u64>,

    /// Degeneracy threshold (optional)
    #[arg(long)]
    degeneracy_threshold: Option<f64>,

    /// Fitness evaluation algorithm ("ssim" or "mse")
    #[arg(long, default_value = "mse", value_parser = clap::builder::PossibleValuesParser::new(["ssim", "mse"])
    )]
    algorithm: String,

    /// Save frequency (optional)
    #[arg(long)]
    save_frequency: Option<usize>,
}

#[derive(Clone)]
struct Triangle {
    vertices: [[i32; 2]; 3],
    color: [u8; 3],
    opacity: f32,
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
    let opacity_range = Uniform::from(0.0f32..=1.0f32);
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
            let opacity = opacity_range.sample(&mut thread_rng);
            Triangle {
                vertices,
                color,
                opacity,
            }
        })
        .collect()
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
                new_triangle.vertices[i][0] = x.clamp(0, image_size.0 as i32 - 1);
                new_triangle.vertices[i][1] = y.clamp(0, image_size.1 as i32 - 1);
            }
        }
        for i in 0..3 {
            if rng.gen::<f64>() < 0.5 {
                let color_component = new_triangle.color[i] as i32 + rng.gen_range(-10..=10);
                new_triangle.color[i] = color_component.clamp(0, 255) as u8;
            }
        }
        if rng.gen::<f64>() < 0.5 {
            let opacity_change = rng.gen_range(-0.1f32..=0.1f32);
            new_triangle.opacity = (new_triangle.opacity + opacity_change).clamp(0.0, 1.0);
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
    let child_opacity = if rng.gen::<f64>() < 0.5 {
        parent1.opacity
    } else {
        parent2.opacity
    };
    Triangle {
        vertices: child_vertices,
        color: child_color,
        opacity: child_opacity,
    }
}

fn evaluate_fitness_batch(
    population: &[Triangle],
    canvas_image: &RgbaImage,
    reference_image: &RgbaImage,
    fitness_algorithm: &str,
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
                if fitness_algorithm == "mse" {
                    -compute_mse(&working_image, reference_image)
                } else {
                    compute_ssim(&working_image, reference_image)
                }
            }
        })
        .collect()
}

fn draw_triangle_onto_canvas(image: &mut RgbaImage, triangle: &Triangle) {
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

    let color = Rgba([
        triangle.color[0],
        triangle.color[1],
        triangle.color[2],
        (255.0 * triangle.opacity) as u8,
    ]);
    draw_polygon_mut(image, &points, color);
}

fn compute_mse(image1: &RgbaImage, image2: &RgbaImage) -> f64 {
    assert_eq!(
        image1.dimensions(),
        image2.dimensions(),
        "Images must have the same dimensions."
    );

    let (width, height) = image1.dimensions();
    let total_pixels = (width * height * 3) as f64;

    let pixels1 = image1.pixels().flat_map(|p| p.0).collect::<Vec<u8>>();
    let pixels2 = image2.pixels().flat_map(|p| p.0).collect::<Vec<u8>>();

    let sum_squared_diff: f64 = pixels1
        .par_iter()
        .zip(pixels2.par_iter())
        .map(|(&c1, &c2)| {
            let diff = c1 as f64 - c2 as f64;
            diff * diff
        })
        .sum();

    sum_squared_diff / total_pixels
}

fn compute_ssim(image1: &RgbaImage, image2: &RgbaImage) -> f64 {
    assert_eq!(
        image1.dimensions(),
        image2.dimensions(),
        "Images must have the same dimensions."
    );

    let (width, height) = image1.dimensions();
    let k1 = 0.01;
    let k2 = 0.03;
    let l: f64 = 255.0;
    let c1 = (k1 * l).powi(2);
    let c2 = (k2 * l).powi(2);

    let ssim_sum: f64 = image1
        .pixels()
        .zip(image2.pixels())
        .par_bridge()
        .map(|(p1, p2)| {
            let mean1: f64 = p1.0.iter().map(|&c| c as f64).sum::<f64>() / 3.0;
            let mean2: f64 = p2.0.iter().map(|&c| c as f64).sum::<f64>() / 3.0;

            let variance1: f64 =
                p1.0.iter()
                    .map(|&c| ((c as f64) - mean1).powi(2))
                    .sum::<f64>()
                    / 3.0;
            let variance2: f64 =
                p2.0.iter()
                    .map(|&c| ((c as f64) - mean2).powi(2))
                    .sum::<f64>()
                    / 3.0;

            let covariance: f64 =
                p1.0.iter()
                    .zip(&p2.0)
                    .map(|(&c1, &c2)| (c1 as f64 - mean1) * (c2 as f64 - mean2))
                    .sum::<f64>()
                    / 3.0;

            let numerator: f64 = (2.0 * mean1 * mean2 + c1) * (2.0 * covariance + c2);
            let denominator: f64 =
                (mean1.powi(2) + mean2.powi(2) + c1) * (variance1 + variance2 + c2);

            numerator / denominator
        })
        .sum();

    ssim_sum / (width * height) as f64
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

    let polygon = Polygon::new()
        .set("points", points)
        .set("fill", color)
        .set("fill-opacity", triangle.opacity);

    *document = document.clone().add(polygon);
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let image_size = (args.image_size, args.image_size);
    let num_triangles = args.num_triangles;
    let num_generations = args.num_generations;
    let population_size = args.population_size;
    let num_selected = args.num_selected;
    let mutation_rate = args.mutation_rate;
    let degeneracy_threshold = args.degeneracy_threshold;
    let save_frequency = args.save_frequency.unwrap_or(10);

    let seed = get_seed(args.seed);
    let mut rng = StdRng::seed_from_u64(seed);

    let reference_image_path = args.reference_image_path;
    let fitness_algorithm = args.algorithm;

    let filename_without_extension = reference_image_path.split('.').next().unwrap();

    let output_svg_path = format!(
        "{}--alg_{}--rng_{}--res_{}--tri_{}--gen_{}--pop_{}--sel_{}--mut_{:.2}--deg_{:.2}.svg",
        filename_without_extension,
        fitness_algorithm,
        seed,
        args.image_size,
        args.num_triangles,
        args.num_generations,
        args.population_size,
        args.num_selected,
        args.mutation_rate,
        args.degeneracy_threshold.unwrap_or(0.0)
    );

    let reference_image = image::open(reference_image_path)?
        .resize_exact(
            image_size.0,
            image_size.1,
            image::imageops::FilterType::Lanczos3,
        )
        .to_rgba8();

    let mut canvas_image = RgbaImage::new(image_size.0, image_size.1);

    let mut document = Document::new().set("viewBox", (0, 0, image_size.0, image_size.1));
    document = document.add(
        Rectangle::new()
            .set("x", 0)
            .set("y", 0)
            .set("width", image_size.0)
            .set("height", image_size.1)
            .set("fill", "black"),
    );

    let pb = ProgressBar::new(num_triangles as u64)
        .with_style(ProgressStyle::default_bar().template("{bar:40.cyan/blue} {pos}/{len}"));

    for triangle_index in 0..num_triangles {
        let mut population = generate_initial_population(population_size, image_size, &mut rng);

        let mut best_triangle = None;
        let mut best_fitness = f64::MIN;

        for _ in 0..num_generations {
            let fitness_scores = evaluate_fitness_batch(
                &population,
                &canvas_image,
                &reference_image,
                &fitness_algorithm,
                degeneracy_threshold.unwrap_or(0.0),
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
        }

        if let Some(ref triangle) = best_triangle {
            draw_triangle_onto_canvas(&mut canvas_image, triangle);
            add_triangle_to_svg(&mut document, triangle);
        }

        if save_frequency > 0 && triangle_index % save_frequency == 0 {
            svg::save(&output_svg_path, &document)?;
        }

        pb.inc(1);
    }

    svg::save(&output_svg_path, &document)?;

    Ok(())
}
