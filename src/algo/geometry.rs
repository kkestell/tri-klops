use super::{AlgorithmParams, raster::Vertices};
use image::RgbImage;
use rand::{RngExt, rngs::SmallRng};

pub(super) fn initial_shape(
    reference: &RgbImage,
    canvas: &RgbImage,
    rng: &mut SmallRng,
) -> Vertices {
    let dimensions = [reference.width() as i32, reference.height() as i32];
    if rng.random_bool(0.5) {
        return std::array::from_fn(|_| dimensions.map(|size| rng.random_range(0..=size)));
    }

    // Center local proposals near a poorly matched pixel. Several scales let
    // the same search cover large silhouettes and small remaining details.
    let center = (0..8)
        .map(|_| dimensions.map(|size| rng.random_range(0..size)))
        .max_by_key(|&[x, y]| {
            let target = reference.get_pixel(x as u32, y as u32);
            let current = canvas.get_pixel(x as u32, y as u32);
            (0..3)
                .map(|c| (i32::from(target[c]) - i32::from(current[c])).pow(2))
                .sum::<i32>()
        })
        .unwrap();
    let shift = rng.random_range(0..=4);
    let radii = dimensions.map(|size| (size >> shift).max(1));
    std::array::from_fn(|_| {
        std::array::from_fn(|axis| {
            let radius = radii[axis];
            center[axis] + rng.random_range(-radius..=radius)
        })
    })
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

pub(super) struct ShapeRules {
    spacing: i32,
    max_cosine: Option<f64>,
}

impl ShapeRules {
    pub fn new(params: &AlgorithmParams) -> Self {
        Self {
            spacing: i32::from(params.grid_spacing),
            max_cosine: params
                .degeneracy_threshold
                .filter(|&deg| deg > 0.0)
                .map(|deg| deg.min(180.0).to_radians().cos()),
        }
    }

    pub fn snap(&self, vertices: Vertices) -> Vertices {
        let step = self.spacing;
        if step <= 1 {
            return vertices;
        }
        vertices.map(|point| point.map(|v| (v + step / 2).div_euclid(step) * step))
    }

    pub fn allows(&self, vertices: Vertices) -> bool {
        self.max_cosine
            .is_none_or(|limit| meets_angle_limit(vertices, limit))
    }
}

pub(super) fn jitter(
    vertex: &mut [i32; 2],
    dimensions: [i32; 2],
    spacing: u16,
    rng: &mut SmallRng,
) {
    for (coord, size) in vertex.iter_mut().zip(dimensions) {
        let step = (size / 10).max(i32::from(spacing)).max(1);
        *coord = (*coord + rng.random_range(-step..=step)).clamp(-size, 2 * size);
    }
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
