use super::geometry::{ShapeRules, jitter};
use super::raster::{Candidate, ScoreRows, Vertices};
use super::{AlgorithmParams, SearchMode, Triangle};
use rand::{RngExt, rngs::SmallRng};
#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;

const GREEDY_MAX_STARTS: usize = 4;
const GREEDY_GENERATIONS_PER_START: usize = 32;

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
    let starts = match params.mode {
        SearchMode::Greedy => {
            (params.num_generations / GREEDY_GENERATIONS_PER_START).clamp(1, GREEDY_MAX_STARTS)
        }
        SearchMode::WholeDrawing => 1,
    };
    let generations_per_start = params.num_generations.div_ceil(starts);
    let rules = ShapeRules::new(params);
    let dimensions = [rows.dimensions.0 as i32, rows.dimensions.1 as i32];
    let mut best = None;
    let mut shapes = Vec::with_capacity(params.population_size);
    let mut ranked = Vec::with_capacity(params.population_size);
    for generation in 0..params.num_generations {
        if generation.is_multiple_of(generations_per_start) {
            shapes.clear();
            shapes.resize_with(params.population_size, || Proposal::New(seed_shape(rng)));
        } else {
            let parents = &ranked[..params.num_selected.min(ranked.len())];
            for shape in &mut shapes {
                *shape = breed(parents, dimensions, params, rng);
            }
        }

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
        if !progress(generation, best).await {
            return best;
        }
    }
    best.map(|best| refine_vertices(best, rows, &rules, params.grid_spacing))
}

fn refine_vertices(
    mut best: Candidate,
    rows: &ScoreRows,
    rules: &ShapeRules,
    grid_spacing: u16,
) -> Candidate {
    let grid = i32::from(grid_spacing).max(1);
    let longest_side = rows.dimensions.0.max(rows.dimensions.1) as i32;
    let max_steps = (longest_side / 16 / grid).max(1);
    // Finish with grid-sized moves so that near misses can still improve.
    for shift in (0..=max_steps.ilog2()).rev() {
        let step = grid << shift;
        let refined = refine_at_step(best, rows, rules, step);
        if refined.gain > best.gain {
            // A move can make a different corner worth revisiting at this scale.
            best = refine_at_step(refined, rows, rules, step);
        }
    }
    best
}

fn refine_at_step(
    mut best: Candidate,
    rows: &ScoreRows,
    rules: &ShapeRules,
    step: i32,
) -> Candidate {
    let offsets = [
        [-step, 0],
        [step, 0],
        [0, -step],
        [0, step],
        [-step, -step],
        [-step, step],
        [step, -step],
        [step, step],
    ];
    for corner in 0..3 {
        let start = best.triangle.vertices;
        for [dx, dy] in offsets {
            let mut vertices = start;
            vertices[corner][0] += dx;
            vertices[corner][1] += dy;
            if !rules.allows(vertices) {
                continue;
            }
            let candidate = rows.fit(vertices);
            if candidate.gain > best.gain {
                best = candidate;
            }
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
    #[cfg(not(target_arch = "wasm32"))]
    fn greedy_keeps_the_best_triangle_across_fresh_populations() {
        use rand::SeedableRng;

        let canvas = RgbImage::new(32, 32);
        let mut reference = canvas.clone();
        let small = Triangle {
            vertices: [[2, 2], [8, 2], [2, 8]],
            color: [80; 3],
        };
        let large = Triangle {
            vertices: [[12, 12], [30, 12], [12, 30]],
            color: [180; 3],
        };
        small.paint(&mut reference);
        large.paint(&mut reference);
        let rows = ScoreRows::new(&reference, &canvas);
        let params = AlgorithmParams {
            num_generations: 128,
            population_size: 1,
            mutation_rate: 0.0,
            ..AlgorithmParams::for_mode(SearchMode::Greedy)
        };
        let mut seeds = [small, small, large, small].into_iter();
        let mut rounds = 0;
        let best = pollster::block_on(best_triangle(
            &params,
            &rows,
            &mut SmallRng::seed_from_u64(42),
            |_| seeds.next().unwrap().vertices,
            async |generation, best| {
                assert_eq!(generation, rounds);
                rounds += 1;
                let expected = if generation < 64 { small } else { large };
                assert_eq!(best.unwrap().triangle, expected);
                true
            },
        ));
        assert_eq!(rounds, params.num_generations);
        assert_eq!(seeds.next(), None);
        assert_eq!(best.unwrap().triangle, large);
    }

    #[test]
    fn refinement_recovers_a_corner_one_grid_step_away() {
        let canvas = RgbImage::new(32, 32);
        let mut reference = canvas.clone();
        let target = Triangle {
            vertices: [[4, 4], [28, 4], [4, 28]],
            color: [200, 100, 50],
        };
        target.paint(&mut reference);
        let rows = ScoreRows::new(&reference, &canvas);
        let initial = rows.fit([[4, 4], [26, 6], [4, 28]]);
        let params = AlgorithmParams {
            grid_spacing: 2,
            ..Default::default()
        };
        let refined = refine_vertices(
            initial,
            &rows,
            &ShapeRules::new(&params),
            params.grid_spacing,
        );
        assert!(initial.gain < refined.gain);
        assert_eq!(refined.gain, rows.error);
        assert_eq!(refined.triangle, target);
    }

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
