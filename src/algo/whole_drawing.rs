mod drawing;

use super::geometry::initial_shape;
use super::raster::{Candidate, ScoreRows, Vertices};
use super::{AlgorithmParams, Progress, Triangle, search};
use drawing::Drawing;
use image::RgbImage;
use rand::{RngExt, rngs::SmallRng};
use std::sync::Mutex;

const SHAPE_GENERATIONS: usize = 32;
const REFINE_INTERVAL: usize = 16;
const MAX_RADIUS_SHIFT: u32 = 8;

fn nudge(
    mut vertices: Vertices,
    dimensions: [i32; 2],
    spacing: u16,
    rng: &mut SmallRng,
) -> Vertices {
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
    vertices
}

async fn refine_layer(
    params: &AlgorithmParams,
    reference: &RgbImage,
    canvas: &RgbImage,
    drawing: &mut Drawing,
    scratch: &mut RgbImage,
    rng: &mut SmallRng,
    keep_searching: impl AsyncFnMut(usize, Option<Candidate>) -> bool,
) -> bool {
    let layer = rng.random_range(0..drawing.triangles.len());
    let original = drawing.triangles.remove(layer);
    drawing.paint(scratch);
    // Layers above the edit keep covering their pixels, regardless of the proposed shape.
    let rows = ScoreRows::under_layers(reference, scratch, &drawing.triangles[layer..]);
    let original_gain = rows.fit(original.vertices).gain;
    let dimensions = [reference.width() as i32, reference.height() as i32];
    let best = search::best_triangle(
        params,
        &rows,
        rng,
        |rng| {
            if rng.random_bool(params.mutation_rate) {
                nudge(original.vertices, dimensions, params.grid_spacing, rng)
            } else {
                initial_shape(reference, canvas, MAX_RADIUS_SHIFT, rng)
            }
        },
        keep_searching,
    )
    .await
    .filter(|best| best.gain > original_gain);

    drawing
        .triangles
        .insert(layer, best.map_or(original, |best| best.triangle));
    best.is_some()
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
    let mut rows = ScoreRows::new(reference, &canvas);
    let mut drawing = Drawing {
        background,
        error: rows.error,
        ..Drawing::default()
    };
    progress.lock().unwrap().publish_drawing(
        &canvas,
        drawing.error,
        background,
        &drawing.triangles,
    );
    checkpoint(progress).await;
    if params.num_triangles == 0 || params.num_generations == 0 || drawing.error == 0 {
        return drawing.triangles;
    }

    let target_squared = reference
        .as_raw()
        .iter()
        .map(|&v| i64::from(v).pow(2))
        .sum();
    let mut owners = vec![0; reference.as_raw().len() / 3];
    let mut scratch = canvas.clone();
    let mut search_params = params.clone();
    search_params.num_generations = SHAPE_GENERATIONS;
    let mut needs_refit = false;
    for generation in 0..params.num_generations {
        if drawing.error == 0 || progress.lock().unwrap().should_stop {
            break;
        }
        let mut keep_searching = async |_, _| {
            checkpoint(progress).await;
            !progress.lock().unwrap().should_stop
        };
        let best = if drawing.triangles.len() < params.num_triangles {
            search::best_triangle(
                &search_params,
                &rows,
                rng,
                |rng| initial_shape(reference, &canvas, MAX_RADIUS_SHIFT, rng),
                &mut keep_searching,
            )
            .await
        } else {
            None
        };
        if let Some(best) = best {
            drawing.triangles.push(best.triangle);
            rows.accept(best, &mut canvas);
            drawing.error = rows.error;
            needs_refit = true;
        }

        let mut changed = best.is_some();
        // Keep adding detail while putting regular work into existing layers.
        if drawing.error > 0
            && !drawing.triangles.is_empty()
            && (best.is_none() || (generation + 1).is_multiple_of(REFINE_INTERVAL))
            && !progress.lock().unwrap().should_stop
            && refine_layer(
                &search_params,
                reference,
                &canvas,
                &mut drawing,
                &mut scratch,
                rng,
                keep_searching,
            )
            .await
        {
            drawing.fit(reference, target_squared, &mut owners);
            drawing.paint(&mut canvas);
            rows = ScoreRows::new(reference, &canvas);
            needs_refit = false;
            changed = true;
        }

        {
            let mut progress = progress.lock().unwrap();
            progress.generation_index = generation;
            if changed {
                progress.publish_drawing(&canvas, drawing.error, background, &drawing.triangles);
            }
        }
        checkpoint(progress).await;
    }

    if needs_refit {
        drawing.fit(reference, target_squared, &mut owners);
        drawing.paint(&mut canvas);
        progress.lock().unwrap().publish_drawing(
            &canvas,
            drawing.error,
            background,
            &drawing.triangles,
        );
        checkpoint(progress).await;
    }
    drawing.triangles
}

#[cfg(test)]
mod tests;
