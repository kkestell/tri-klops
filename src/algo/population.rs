mod drawing;

use super::geometry::{ShapeRules, initial_shape};
use super::{AlgorithmParams, Progress, Triangle};
use drawing::Drawing;
use image::{Rgb, RgbImage};
use rand::{RngExt, rngs::SmallRng};
#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;
use std::sync::Mutex;

fn crossover(parents: &[Drawing], limit: usize, rng: &mut SmallRng) -> Vec<Triangle> {
    let a = &parents[rng.random_range(0..parents.len())].triangles;
    let b = &parents[rng.random_range(0..parents.len())].triangles;
    let cut_a = rng.random_range(0..=a.len());
    let cut_b = rng.random_range(0..=b.len());
    let mut triangles = a[..cut_a].to_vec();
    triangles.extend(b[cut_b..].iter().take(limit - cut_a));
    triangles
}

fn nudge(
    triangle: &mut Triangle,
    dimensions: [i32; 2],
    spacing: u16,
    rules: &ShapeRules,
    rng: &mut SmallRng,
) {
    let mut vertices = triangle.vertices;
    let vertex = &mut vertices[rng.random_range(0..3)];
    let axis = rng.random_range(0..2);
    let coord = &mut vertex[axis];
    let size = dimensions[axis];
    let grid = i32::from(spacing).max(1);
    // Sample powers of two so a single grid step remains common on large canvases.
    let max_steps = (size / 10 / grid).max(1);
    let distance = grid << rng.random_range(0..=max_steps.ilog2());
    let delta = if rng.random_bool(0.5) {
        distance
    } else {
        -distance
    };
    *coord = (*coord + delta).clamp(-size, 2 * size);
    let vertices = rules.snap(vertices);
    if rules.allows(vertices) {
        triangle.vertices = vertices;
    }
}

fn offspring(
    parents: &[Drawing],
    params: &AlgorithmParams,
    rules: &ShapeRules,
    reference: &RgbImage,
    canvas: &RgbImage,
    rng: &mut SmallRng,
) -> Drawing {
    let best = &parents[0].triangles;
    // With few layers crossover is cheap. As the expected number of mutations
    // grows, favor editing one part of the leader without disturbing the rest.
    let expected_mutations = best.len() as f64 * params.mutation_rate;
    let edit_leader = rng.random_bool(expected_mutations / (1.0 + expected_mutations));
    let refine = edit_leader && rng.random_bool(0.5);
    let mut triangles = if edit_leader {
        best.clone()
    } else {
        crossover(parents, params.num_triangles, rng)
    };
    if refine {
        let index = rng.random_range(0..triangles.len());
        nudge(
            &mut triangles[index],
            [reference.width() as i32, reference.height() as i32],
            params.grid_spacing,
            rules,
            rng,
        );
    } else {
        let vertices = rules.snap(initial_shape(reference, canvas, rng));
        if rules.allows(vertices) {
            if triangles.len() == params.num_triangles {
                triangles.remove(rng.random_range(0..triangles.len()));
            }
            triangles.push(Triangle {
                vertices,
                color: [0; 3],
            });
        }
    }
    Drawing {
        triangles,
        error: 0,
    }
}

pub(super) async fn evolve(
    params: &AlgorithmParams,
    reference: &RgbImage,
    mut canvas: RgbImage,
    progress: &Mutex<Progress>,
    rng: &mut SmallRng,
    checkpoint: &mut impl AsyncFnMut(&Mutex<Progress>),
) -> Vec<Triangle> {
    let background = canvas.get_pixel(0, 0).0;
    let target_squared = reference
        .as_raw()
        .iter()
        .map(|&v| i64::from(v).pow(2))
        .sum();
    let pixels = reference.as_raw().len() / 3;
    let mut empty = Drawing::default();
    empty.fit(reference, background, target_squared, &mut vec![0; pixels]);
    progress
        .lock()
        .unwrap()
        .publish_drawing(&canvas, empty.error, background, &empty.triangles);
    checkpoint(progress).await;
    let mut parents = vec![empty];
    let rules = ShapeRules::new(params);
    #[cfg(target_arch = "wasm32")]
    let mut owners = vec![0; pixels];

    for generation in 0..params.num_generations {
        let previous_error = parents[0].error;
        if previous_error == 0 || params.num_triangles == 0 || progress.lock().unwrap().should_stop
        {
            break;
        }
        let mut children: Vec<_> = (0..params.population_size)
            .map(|_| offspring(&parents, params, &rules, reference, &canvas, rng))
            .collect();
        #[cfg(not(target_arch = "wasm32"))]
        children.par_iter_mut().for_each_init(
            || vec![0; pixels],
            |owners, drawing| {
                drawing.fit(reference, background, target_squared, owners);
            },
        );
        #[cfg(target_arch = "wasm32")]
        for drawing in &mut children {
            drawing.fit(reference, background, target_squared, &mut owners);
        }
        parents.append(&mut children);
        parents.sort_by_key(|drawing| drawing.error);
        parents.truncate(params.num_selected.min(params.population_size));

        let best = &parents[0];
        let improved = best.error < previous_error;
        if improved {
            canvas.pixels_mut().for_each(|p| *p = Rgb(background));
            for triangle in &best.triangles {
                triangle.paint(&mut canvas);
            }
        }
        {
            let mut progress = progress.lock().unwrap();
            progress.generation_index = generation;
            if improved {
                progress.publish_drawing(&canvas, best.error, background, &best.triangles);
            }
        }
        checkpoint(progress).await;
    }
    parents.swap_remove(0).triangles
}

#[cfg(test)]
mod tests;
