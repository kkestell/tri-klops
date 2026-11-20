use super::AlgorithmParams;
use super::raster::{Candidate, ScoreRows, Vertices};
use image::RgbImage;
use rand::{RngExt, rngs::SmallRng};
use rayon::prelude::*;

fn initial_shape(reference: &RgbImage, canvas: &RgbImage, rng: &mut SmallRng) -> Vertices {
    let size = reference.width() as i32;
    if rng.random_bool(0.5) {
        return std::array::from_fn(|_| [rng.random_range(0..=size), rng.random_range(0..=size)]);
    }

    // Center local proposals near a poorly matched pixel. Several scales let
    // the same search cover large silhouettes and small remaining details.
    let center = (0..8)
        .map(|_| [rng.random_range(0..size), rng.random_range(0..size)])
        .max_by_key(|&[x, y]| {
            let target = reference.get_pixel(x as u32, y as u32);
            let current = canvas.get_pixel(x as u32, y as u32);
            (0..3)
                .map(|c| (i32::from(target[c]) - i32::from(current[c])).pow(2))
                .sum::<i32>()
        })
        .unwrap();
    let radius = (size >> rng.random_range(0..=4)).max(1);
    std::array::from_fn(|_| center.map(|v| v + rng.random_range(-radius..=radius)))
}

fn meets_angle_limit(vertices: Vertices, max_cosine: f64) -> bool {
    (0..3).all(|i| {
        let origin = vertices[i];
        let a = vertices[(i + 1) % 3];
        let b = vertices[(i + 2) % 3];
        let ax = f64::from(a[0] - origin[0]);
        let ay = f64::from(a[1] - origin[1]);
        let bx = f64::from(b[0] - origin[0]);
        let by = f64::from(b[1] - origin[1]);
        let lengths = ((ax * ax + ay * ay) * (bx * bx + by * by)).sqrt();
        lengths > 0.0 && ax * bx + ay * by < max_cosine * lengths
    })
}

pub(super) fn best_triangle(
    params: &AlgorithmParams,
    reference: &RgbImage,
    canvas: &RgbImage,
    rows: &ScoreRows,
    rng: &mut SmallRng,
    mut progress: impl FnMut(usize, Option<Candidate>) -> bool,
) -> Option<Candidate> {
    let mut shapes: Vec<_> = (0..params.population_size)
        .map(|_| initial_shape(reference, canvas, rng))
        .collect();
    let cosine = params
        .degeneracy_threshold
        .filter(|&deg| deg > 0.0)
        .map(|deg| deg.min(180.0).to_radians().cos());
    let size = reference.width() as i32;
    let mut best = None;
    let mut ranked = Vec::with_capacity(shapes.len());
    for generation in 0..params.num_generations {
        shapes
            .par_iter()
            .map(|&vertices| {
                if cosine.is_some_and(|limit| !meets_angle_limit(vertices, limit)) {
                    Candidate {
                        triangle: super::Triangle {
                            vertices,
                            color: [0; 3],
                        },
                        gain: i64::MIN,
                    }
                } else {
                    rows.fit(vertices)
                }
            })
            .collect_into_vec(&mut ranked);
        ranked.sort_unstable_by_key(|c| std::cmp::Reverse(c.gain));
        let leader = ranked[0];
        if leader.gain > best.map_or(0, |c: Candidate| c.gain) {
            best = Some(leader);
        }
        if !progress(generation, best) || generation + 1 == params.num_generations {
            break;
        }
        let parents = &ranked[..params.num_selected.min(ranked.len())];
        for shape in &mut shapes {
            *shape = breed(parents, size, params.mutation_rate, rng);
        }
    }
    best
}

fn breed(parents: &[Candidate], size: i32, mutation_rate: f64, rng: &mut SmallRng) -> Vertices {
    let mut parent = || {
        parents[rng.random_range(0..parents.len())]
            .triangle
            .vertices
    };
    let a = parent();
    let b = parent();
    let step = (size / 10).max(1);
    std::array::from_fn(|i| {
        let mut vertex = if rng.random_bool(0.5) { a[i] } else { b[i] };
        if rng.random_bool(mutation_rate) {
            for coord in &mut vertex {
                *coord = (*coord + rng.random_range(-step..=step)).clamp(-size, 2 * size);
            }
        }
        vertex
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_checks_each_corner_and_rejects_coincident_vertices() {
        let right = [[0, 0], [0, 6], [6, 0]];
        assert!(meets_angle_limit(right, 44f64.to_radians().cos()));
        assert!(!meets_angle_limit(right, 46f64.to_radians().cos()));
        assert!(!meets_angle_limit([[0, 0], [0, 0], [2, 2]], 0.99));
    }
}
