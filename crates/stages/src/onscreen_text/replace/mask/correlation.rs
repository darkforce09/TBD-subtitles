//! Zero-mean normalized cross-correlation of grey templates.
//!
//! **Role:** score how well a template matches a grey image at a position, independent of the
//! image's brightness and contrast there.
//! **Position:** the matching arithmetic behind following moving writing.
//! **Signals and state:** floating-point planes, their running sums and zero-mean templates.
//! **Invariants:** scores lie in [-1, 1]; flat templates never exist and flat image windows
//! score zero.

use image::{GrayImage, Luma, RgbImage};

/// A grey image in floating point.
pub(super) struct Plane {
    pub w: usize,
    pub h: usize,
    data: Vec<f32>,
}

impl Plane {
    pub(super) fn from_gray(image: &GrayImage) -> Plane {
        Plane {
            w: image.width() as usize,
            h: image.height() as usize,
            data: image.pixels().map(|p| f32::from(p.0[0])).collect(),
        }
    }

    /// Box-average `factor` × `factor` blocks; a trailing partial block is dropped.
    pub(super) fn shrink(&self, factor: usize) -> Plane {
        let (w, h) = (self.w / factor, self.h / factor);
        let area = (factor * factor) as f32;
        let mut data = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let mut sum = 0.0;
                for row in 0..factor {
                    let start = (y * factor + row) * self.w + x * factor;
                    sum += self.data[start..start + factor].iter().sum::<f32>();
                }
                data.push(sum / area);
            }
        }
        Plane { w, h, data }
    }
}

/// Running sums of a plane and its squares for window statistics.
pub(super) struct Integral {
    stride: usize,
    sum: Vec<f64>,
    squares: Vec<f64>,
}

impl Integral {
    pub(super) fn new(plane: &Plane) -> Integral {
        let stride = plane.w + 1;
        let mut sum = vec![0.0; stride * (plane.h + 1)];
        let mut squares = sum.clone();
        for y in 0..plane.h {
            let (mut row, mut row_squares) = (0.0, 0.0);
            for x in 0..plane.w {
                let v = f64::from(plane.data[y * plane.w + x]);
                row += v;
                row_squares += v * v;
                sum[(y + 1) * stride + x + 1] = sum[y * stride + x + 1] + row;
                squares[(y + 1) * stride + x + 1] = squares[y * stride + x + 1] + row_squares;
            }
        }
        Integral {
            stride,
            sum,
            squares,
        }
    }

    /// Sum and sum of squares of the `w` × `h` window at `(x, y)`.
    fn window(&self, x: usize, y: usize, w: usize, h: usize) -> (f64, f64) {
        let area = |t: &[f64]| {
            let at = |xx: usize, yy: usize| t[yy * self.stride + xx];
            at(x + w, y + h) - at(x, y + h) - at(x + w, y) + at(x, y)
        };
        (area(&self.sum), area(&self.squares))
    }
}

/// A zero-mean template.
pub(super) struct Template {
    pub w: usize,
    pub h: usize,
    values: Vec<f32>,
    norm: f64,
}

impl Template {
    /// `None` for a template smaller than 2 × 2 or without contrast.
    pub(super) fn new(plane: &Plane) -> Option<Template> {
        let n = plane.data.len();
        if plane.w < 2 || plane.h < 2 {
            return None;
        }
        let mean = plane.data.iter().map(|&v| f64::from(v)).sum::<f64>() / n as f64;
        let values: Vec<f32> = plane
            .data
            .iter()
            .map(|&v| (f64::from(v) - mean) as f32)
            .collect();
        let norm = values
            .iter()
            .map(|&v| f64::from(v).powi(2))
            .sum::<f64>()
            .sqrt();
        (norm > 0.5 * (n as f64).sqrt()).then_some(Template {
            w: plane.w,
            h: plane.h,
            values,
            norm,
        })
    }

    /// Zero-mean normalized cross-correlation with `plane` at top-left `(x, y)`; the template
    /// must fit there.
    pub(super) fn score(&self, plane: &Plane, integral: &Integral, x: usize, y: usize) -> f32 {
        let n = (self.w * self.h) as f64;
        let (sum, squares) = integral.window(x, y, self.w, self.h);
        let variance = squares - sum * sum / n;
        if variance <= 0.25 * n {
            return 0.0;
        }
        let mut dot = 0.0f64;
        for row in 0..self.h {
            let image = &plane.data[(y + row) * plane.w + x..][..self.w];
            let template = &self.values[row * self.w..][..self.w];
            dot += f64::from(image.iter().zip(template).map(|(a, b)| a * b).sum::<f32>());
        }
        (dot / (self.norm * variance.sqrt())) as f32
    }
}

/// Luma of an RGB image with Rec. 601 weights.
pub(super) fn grey(image: &RgbImage) -> GrayImage {
    GrayImage::from_fn(image.width(), image.height(), |x, y| {
        let [r, g, b] = image.get_pixel(x, y).0.map(f64::from);
        Luma([(0.299 * r + 0.587 * g + 0.114 * b).round() as u8])
    })
}

#[cfg(test)]
#[path = "tests/correlation.rs"]
mod tests;
