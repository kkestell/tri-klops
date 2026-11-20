use super::geometry::{ShapeRules, jitter};
use super::raster::{Candidate, ScoreRows, Vertices};
use super::{AlgorithmParams, Triangle};
use rand::{RngExt, rngs::SmallRng};
#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;

#[derive(Clone, Copy)]
enum Proposal {
    New(Vertices),
    Inherited(Candidate),
}

impl Proposal {
    fn evaluate(self, rows: &ScoreRows, rules: &ShapeRules) -> Candidate {
        let vertices = match self {
            Self::Inherited(parent) => return parent,
            Self::New(vertices) => rules.snap(vertices),
        };
        if !rules.allows(vertices) {
            return Candidate {
                triangle: Triangle {
                    vertices,
                    color: [0; 3],
                },
                gain: i64::MIN,
            };
        }
        rows.fit(vertices)
    }
}

pub(super) async fn best_triangle(
    params: &AlgorithmParams,
    rows: &ScoreRows,
    rng: &mut SmallRng,
    mut seed_shape: impl FnMut(&mut SmallRng) -> Vertices,
    mut progress: impl AsyncFnMut(usize, Option<Candidate>) -> bool,
) -> Option<Candidate> {
    let mut shapes: Vec<_> = (0..params.population_size)
        .map(|_| Proposal::New(seed_shape(rng)))
        .collect();
    let rules = ShapeRules::new(params);
    let dimensions = [rows.dimensions.0 as i32, rows.dimensions.1 as i32];
    let mut best = None;
    let mut ranked = Vec::with_capacity(shapes.len());
    for generation in 0..params.num_generations {
        // Small populations cost more to split across workers than to score locally.
        #[cfg(not(target_arch = "wasm32"))]
        shapes
            .par_iter()
            .with_min_len(128)
            .map(|shape| shape.evaluate(rows, &rules))
            .collect_into_vec(&mut ranked);
        #[cfg(target_arch = "wasm32")]
        {
            ranked.clear();
            ranked.extend(shapes.iter().map(|shape| shape.evaluate(rows, &rules)));
        }
        ranked.sort_unstable_by_key(|c| std::cmp::Reverse(c.gain));
        let leader = ranked[0];
        if leader.gain > best.map_or(0, |c: Candidate| c.gain) {
            best = Some(leader);
        }
        if !progress(generation, best).await || generation + 1 == params.num_generations {
            break;
        }
        let parents = &ranked[..params.num_selected.min(ranked.len())];
        for shape in &mut shapes {
            *shape = breed(parents, dimensions, params, rng);
        }
    }
    best
}

fn breed(
    parents: &[Candidate],
    dimensions: [i32; 2],
    params: &AlgorithmParams,
    rng: &mut SmallRng,
) -> Proposal {
    let mut parent = || parents[rng.random_range(0..parents.len())];
    let a = parent();
    let b = parent();
    let vertices = std::array::from_fn(|i| {
        let mut vertex = if rng.random_bool(0.5) {
            a.triangle.vertices[i]
        } else {
            b.triangle.vertices[i]
        };
        if rng.random_bool(params.mutation_rate) {
            jitter(&mut vertex, dimensions, params.grid_spacing, rng);
        }
        vertex
    });
    if vertices == a.triangle.vertices {
        Proposal::Inherited(a)
    } else if vertices == b.triangle.vertices {
        Proposal::Inherited(b)
    } else {
        Proposal::New(vertices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbImage;

    #[test]
    fn fit_and_angle_check_use_snapped_vertices() {
        let image = RgbImage::new(16, 16);
        let rows = ScoreRows::new(&image, &image);
        let proposal = Proposal::New([[-3, 1], [6, 3], [2, 7]]);
        let snapped = [[-4, 0], [8, 4], [4, 8]];
        let mut params = AlgorithmParams {
            grid_spacing: 4,
            ..Default::default()
        };
        let fitted = proposal.evaluate(&rows, &ShapeRules::new(&params));
        assert_eq!(fitted.triangle, rows.fit(snapped).triangle);
        assert_eq!(fitted.gain, rows.fit(snapped).gain);
        let collapses = Proposal::New([[3, 3], [4, 4], [13, 6]]);
        params.degeneracy_threshold = Some(1.0);
        assert_eq!(
            collapses.evaluate(&rows, &ShapeRules::new(&params)).gain,
            i64::MIN
        );
    }
}
