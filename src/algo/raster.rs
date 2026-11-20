use image::{Rgb, RgbImage};
use std::ops::Range;

pub(super) type Vertices = [[i32; 2]; 3];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Triangle {
    pub vertices: Vertices,
    pub color: [u8; 3],
}

impl Triangle {
    pub fn paint(&self, canvas: &mut RgbImage) {
        spans(self.vertices, canvas.dimensions(), |y, xs| {
            for x in xs {
                canvas.put_pixel(x as u32, y as u32, Rgb(self.color));
            }
        });
    }
}

// SVG coordinates lie on pixel boundaries. Sample the center of each pixel,
// including the left edge and excluding the right edge and horizontal edges.
fn spans(
    vertices: Vertices,
    (width, height): (u32, u32),
    mut visit: impl FnMut(usize, Range<usize>),
) {
    let mut points = vertices;
    points.sort_unstable_by_key(|p| p[1]);
    let [top, mid, bottom] = points;
    let dx = |p: [i32; 2]| i128::from(p[0]) - i128::from(top[0]);
    let dy = |p: [i32; 2]| i128::from(p[1]) - i128::from(top[1]);
    let area = dx(mid) * dy(bottom) - dx(bottom) * dy(mid);
    if area == 0 {
        return;
    }

    let edge = |a: [i32; 2], b: [i32; 2]| {
        let slope = (f64::from(b[0]) - f64::from(a[0])) / (f64::from(b[1]) - f64::from(a[1]));
        move |y: i32| {
            let x = f64::from(a[0]) + (f64::from(y) - f64::from(a[1]) + 0.5) * slope;
            ((x - 0.5).ceil() as u32).min(width) as usize
        }
    };
    let long = edge(top, bottom);
    for (a, b) in [(top, mid), (mid, bottom)] {
        let rows = a[1].max(0)..b[1].min(height as i32);
        if rows.is_empty() {
            continue;
        }
        let short = edge(a, b);
        let (left, right) = if area > 0 {
            (&long, &short)
        } else {
            (&short, &long)
        };
        for y in rows {
            visit(y as usize, left(y)..right(y));
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Candidate {
    pub triangle: Triangle,
    pub gain: i64,
}

pub(super) struct ScoreRows {
    stride: usize,
    dimensions: (u32, u32),
    prefix: Vec<[i64; 4]>,
    pub error: i64,
}

impl ScoreRows {
    pub fn new(reference: &RgbImage, canvas: &RgbImage) -> Self {
        let stride = reference.width() as usize + 1;
        let mut prefix = Vec::with_capacity(stride * reference.height() as usize);
        let mut error = 0;
        for (target_row, canvas_row) in reference.rows().zip(canvas.rows()) {
            let mut sums = [0; 4];
            prefix.push(sums);
            for (target, current) in target_row.zip(canvas_row) {
                for c in 0..3 {
                    let t = i64::from(target[c]);
                    let diff = t - i64::from(current[c]);
                    sums[c] += t;
                    sums[3] += diff * diff - t * t;
                    error += diff * diff;
                }
                prefix.push(sums);
            }
        }
        Self {
            stride,
            dimensions: reference.dimensions(),
            prefix,
            error,
        }
    }

    // For a fill C over target T, the fourth prefix component accumulates
    // |T - C|² - |T|² = |C|² - 2 C·T. Only this component changes.
    pub fn accept(&mut self, candidate: Candidate, canvas: &mut RgbImage) {
        candidate.triangle.paint(canvas);
        let color = candidate.triangle.color.map(i64::from);
        let norm = color.iter().map(|v| v * v).sum::<i64>();
        spans(candidate.triangle.vertices, self.dimensions, |y, xs| {
            if xs.is_empty() {
                return;
            }
            let row = &mut self.prefix[y * self.stride..(y + 1) * self.stride];
            let start = row[xs.start];
            let old_end = row[xs.end][3];
            for (i, p) in row[xs.start + 1..=xs.end].iter_mut().enumerate() {
                let dot = (0..3).map(|c| color[c] * (p[c] - start[c])).sum::<i64>();
                p[3] = start[3] + (i as i64 + 1) * norm - 2 * dot;
            }
            let delta = row[xs.end][3] - old_end;
            for p in &mut row[xs.end + 1..] {
                p[3] += delta;
            }
        });
        self.error -= candidate.gain;
    }

    pub fn fitness(&self, error: i64) -> f64 {
        let (width, height) = self.dimensions;
        -(error as f64) / (f64::from(width) * f64::from(height) * 3.0)
    }

    pub fn fit(&self, vertices: Vertices) -> Candidate {
        let mut sums = [0; 4];
        let mut count = 0i64;
        spans(vertices, self.dimensions, |y, xs| {
            let row = y * self.stride;
            let start = self.prefix[row + xs.start];
            let end = self.prefix[row + xs.end];
            for i in 0..4 {
                sums[i] += end[i] - start[i];
            }
            count += xs.len() as i64;
        });

        // The rounded mean minimizes squared error for an opaque fill.
        // sums[3] holds old squared error minus the sum of target squares.
        let mut color = [0; 3];
        let mut gain = sums[3];
        if count > 0 {
            for i in 0..3 {
                let mean = (sums[i] + count / 2) / count;
                color[i] = mean as u8;
                gain += mean * (2 * sums[i] - count * mean);
            }
        }
        Candidate {
            triangle: Triangle { vertices, color },
            gain,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scanlines_cover_pixel_centers_and_clip_at_canvas_edges() {
        let mut mask = [[false; 4]; 3];
        spans([[-1, 0], [4, 0], [-1, 5]], (4, 3), |y, xs| {
            mask[y][xs].fill(true);
        });
        assert_eq!(
            mask,
            [
                [true, true, true, false],
                [true, true, false, false],
                [true, false, false, false],
            ]
        );
    }

    #[test]
    fn incremental_scores_match_a_full_rebuild_after_overlapping_fills() {
        let reference = RgbImage::from_fn(12, 9, |x, y| Rgb([(x * 23) as u8, (y * 31) as u8, 120]));
        let mut canvas = RgbImage::from_pixel(12, 9, Rgb([60, 80, 140]));
        let mut rows = ScoreRows::new(&reference, &canvas);
        for vertices in [
            [[-4, 2], [7, -3], [15, 9]],
            [[11, 8], [0, 8], [5, 1]],
            [[12, 0], [5, 9], [-4, 0]],
        ] {
            rows.accept(rows.fit(vertices), &mut canvas);
            let rebuilt = ScoreRows::new(&reference, &canvas);
            assert_eq!(rows.error, rebuilt.error);
            assert_eq!(rows.prefix, rebuilt.prefix);
        }
    }

    #[test]
    fn fitted_color_and_gain_match_brute_force() {
        let reference = RgbImage::from_fn(8, 8, |x, y| Rgb([(x * 23) as u8, (y * 31) as u8, 120]));
        let canvas = RgbImage::from_pixel(8, 8, Rgb([60, 80, 140]));
        let rows = ScoreRows::new(&reference, &canvas);
        let error = |image: &RgbImage| {
            image
                .as_raw()
                .iter()
                .zip(reference.as_raw())
                .map(|(&a, &b)| (i64::from(a) - i64::from(b)).pow(2))
                .sum::<i64>()
        };
        assert_eq!(rows.error, error(&canvas));
        for vertices in [
            [[-8, 4], [7, 0], [12, 10]],
            [[4, 7], [0, 3], [7, 3]],
            [[0, 0], [2, 2], [4, 4]],
            [[-2, -2], [-4, -5], [-1, -5]],
        ] {
            let fitted = rows.fit(vertices);
            let mut painted = canvas.clone();
            fitted.triangle.paint(&mut painted);
            let fitted_error = error(&painted);
            assert_eq!(fitted.gain, error(&canvas) - fitted_error);
            for channel in 0..3 {
                for value in 0..=255 {
                    let mut alternate = fitted.triangle;
                    alternate.color[channel] = value;
                    let mut painted = canvas.clone();
                    alternate.paint(&mut painted);
                    assert!(error(&painted) >= fitted_error);
                }
            }
        }
    }
}
