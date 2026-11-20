use super::Triangle;
use crate::algo::raster::spans;
use image::{Rgb, RgbImage};

#[derive(Default)]
pub(super) struct Drawing {
    pub(super) background: [u8; 3],
    pub(super) triangles: Vec<Triangle>,
    pub(super) error: i64,
}

#[derive(Clone, Default)]
struct Coverage {
    pixels: i64,
    rgb_sum: [i64; 3],
}

impl Coverage {
    // The target-squared term is constant across drawings. This is the
    // remaining contribution to squared error for pixels painted this color.
    fn color_error(&self, color: [u8; 3]) -> i64 {
        color
            .into_iter()
            .zip(self.rgb_sum)
            .map(|(c, sum)| {
                let c = i64::from(c);
                c * (self.pixels * c - 2 * sum)
            })
            .sum()
    }
}

impl Drawing {
    pub(super) fn paint(&self, canvas: &mut RgbImage) {
        canvas
            .pixels_mut()
            .for_each(|pixel| *pixel = Rgb(self.background));
        for triangle in &self.triangles {
            triangle.paint(canvas);
        }
    }

    pub(super) fn fit(&mut self, reference: &RgbImage, target_squared: i64, owners: &mut [usize]) {
        let width = reference.width() as usize;
        owners.fill(0);
        for (index, triangle) in self.triangles.iter().enumerate() {
            spans(triangle.vertices, reference.dimensions(), |y, xs| {
                owners[y * width + xs.start..y * width + xs.end].fill(index + 1);
            });
        }

        // Only the topmost triangle at each pixel contributes to the image.
        let mut coverage = vec![Coverage::default(); self.triangles.len() + 1];
        for (&owner, pixel) in owners.iter().zip(reference.pixels()) {
            let layer = &mut coverage[owner];
            layer.pixels += 1;
            for (sum, value) in layer.rgb_sum.iter_mut().zip(pixel.0) {
                *sum += i64::from(value);
            }
        }
        self.error = target_squared + coverage[0].color_error(self.background);
        let mut layers = coverage[1..].iter();
        self.triangles.retain_mut(|triangle| {
            let layer = layers.next().unwrap();
            if layer.pixels == 0 {
                return false;
            }
            triangle.color = layer
                .rgb_sum
                .map(|sum| ((sum + layer.pixels / 2) / layer.pixels) as u8);
            self.error += layer.color_error(triangle.color);
            true
        });
    }
}
