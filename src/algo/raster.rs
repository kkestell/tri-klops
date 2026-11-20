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
    let dx = |p: [i32; 2]| i64::from(p[0] - top[0]);
    let dy = |p: [i32; 2]| i64::from(p[1] - top[1]);
    if dx(mid) * dy(bottom) == dx(bottom) * dy(mid) {
        return;
    }
    let slope = |a: [i32; 2], b: [i32; 2]| (b[0] - a[0]) as f64 / (b[1] - a[1]) as f64;
    let long_slope = slope(top, bottom);
    let boundary = |x: f64| (x - 0.5).ceil().clamp(0.0, width as f64) as usize;
    for (a, b) in [(top, mid), (mid, bottom)] {
        let rows = a[1].max(0)..b[1].min(height as i32);
        if rows.is_empty() {
            continue;
        }
        let short_slope = slope(a, b);
        for y in rows {
            let left = top[0] as f64 + (f64::from(y - top[1]) + 0.5) * long_slope;
            let right = a[0] as f64 + (f64::from(y - a[1]) + 0.5) * short_slope;
            let xs = boundary(left.min(right))..boundary(left.max(right));
            if !xs.is_empty() {
                visit(y as usize, xs);
            }
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
