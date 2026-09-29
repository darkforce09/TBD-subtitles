//! Checked projective geometry for tracked lettering.
//!
//! **Role:** fit and apply perspective transforms while rejecting degenerate evidence.
//! **Position:** shared by visual tracking and vector typesetting.
//! **Signals and state:** pure point correspondences, matrices and reprojection residuals.
//! **Invariants:** fitted points are finite; RANSAC is deterministic; transforms preserve a
//! convex quadrilateral's orientation rather than folding the text surface.

use job_model::onscreen::{Point, Quad};
use nalgebra::{Matrix3, SMatrix, SVector, Vector3};

pub type Homography = Matrix3<f64>;

#[derive(Debug, Clone)]
pub struct Fit {
    pub homography: Homography,
    pub inliers: Vec<usize>,
    pub rms_error: f64,
    pub max_error: f64,
}

pub fn project(transform: &Homography, point: Point) -> Option<Point> {
    let p = transform * Vector3::new(point.x, point.y, 1.0);
    if !p.iter().all(|n| n.is_finite()) || p.z.abs() < 1e-9 {
        return None;
    }
    let mapped = Point {
        x: p.x / p.z,
        y: p.y / p.z,
    };
    (mapped.x.is_finite() && mapped.y.is_finite()).then_some(mapped)
}

pub fn map_quad(transform: &Homography, quad: Quad) -> Option<Quad> {
    let mut mapped = Quad::default();
    let mut divisor_sign = 0.0f64;
    for (index, point) in quad.0.iter().enumerate() {
        let divisor = transform[(2, 0)] * point.x + transform[(2, 1)] * point.y + transform[(2, 2)];
        if divisor_sign != 0.0 && divisor.signum() != divisor_sign {
            return None;
        }
        divisor_sign = divisor.signum();
        mapped.0[index] = project(transform, *point)?;
    }
    (convex(mapped) && signed_area(mapped) * signed_area(quad) > 0.0).then_some(mapped)
}

pub fn quad_to_quad(source: Quad, target: Quad) -> Option<Homography> {
    if !convex(source) || !convex(target) {
        return None;
    }
    let transform = least_squares(&source.0, &target.0)?;
    let mapped = map_quad(&transform, source)?;
    let error = mapped
        .0
        .iter()
        .zip(target.0)
        .map(|(a, b)| distance(*a, b))
        .fold(0.0, f64::max);
    (error < 1e-5).then_some(transform)
}

/// Robust normalized homography; callers decide the minimum evidence needed for acceptance.
pub fn robust_fit(source: &[Point], target: &[Point], tolerance: f64) -> Option<Fit> {
    if source.len() != target.len()
        || source.len() < 4
        || !tolerance.is_finite()
        || tolerance <= 0.0
    {
        return None;
    }
    if source
        .iter()
        .chain(target)
        .any(|p| !p.x.is_finite() || !p.y.is_finite())
    {
        return None;
    }
    let mut best: Option<Fit> = None;
    let mut random = 0x9e3779b97f4a7c15u64 ^ source.len() as u64;
    for _ in 0..128 {
        let mut indices = [usize::MAX; 4];
        for index in 0..4 {
            loop {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                let candidate = (random % source.len() as u64) as usize;
                if !indices[..index].contains(&candidate) {
                    indices[index] = candidate;
                    break;
                }
            }
        }
        let src = indices.map(|i| source[i]);
        let dst = indices.map(|i| target[i]);
        let Some(transform) = least_squares(&src, &dst) else {
            continue;
        };
        let fit = residuals(transform, source, target, tolerance);
        if best.as_ref().is_none_or(|b| {
            fit.inliers.len() > b.inliers.len()
                || (fit.inliers.len() == b.inliers.len() && fit.rms_error < b.rms_error)
        }) {
            best = Some(fit);
        }
    }
    let mut best = best.filter(|b| b.inliers.len() >= 4)?;
    for _ in 0..2 {
        let src: Vec<_> = best.inliers.iter().map(|&i| source[i]).collect();
        let dst: Vec<_> = best.inliers.iter().map(|&i| target[i]).collect();
        let transform = least_squares(&src, &dst)?;
        best = residuals(transform, source, target, tolerance);
        if best.inliers.len() < 4 {
            return None;
        }
    }
    Some(best)
}

pub fn contains(quad: Quad, point: Point) -> bool {
    if !point.x.is_finite() || !point.y.is_finite() {
        return false;
    }
    let mut positive = false;
    let mut negative = false;
    for i in 0..4 {
        let cross = cross(quad.0[i], quad.0[(i + 1) % 4], point);
        positive |= cross > 1e-6;
        negative |= cross < -1e-6;
    }
    !(positive && negative) && convex(quad)
}

pub fn area(quad: Quad) -> f64 {
    signed_area(quad).abs()
}

pub fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn residuals(transform: Homography, source: &[Point], target: &[Point], tolerance: f64) -> Fit {
    let mut fit = Fit {
        homography: transform,
        inliers: Vec::new(),
        rms_error: 0.0,
        max_error: 0.0,
    };
    for (i, (a, b)) in source.iter().zip(target).enumerate() {
        if let Some(mapped) = project(&transform, *a) {
            let error = distance(mapped, *b);
            if error <= tolerance {
                fit.inliers.push(i);
                fit.rms_error += error * error;
                fit.max_error = fit.max_error.max(error);
            }
        }
    }
    fit.rms_error = if fit.inliers.is_empty() {
        f64::INFINITY
    } else {
        (fit.rms_error / fit.inliers.len() as f64).sqrt()
    };
    fit
}

fn least_squares(source: &[Point], target: &[Point]) -> Option<Homography> {
    if source.len() != target.len() || source.len() < 4 {
        return None;
    }
    let source_normal = normalization(source)?;
    let target_normal = normalization(target)?;
    let mut normal = SMatrix::<f64, 8, 8>::zeros();
    let mut values = SVector::<f64, 8>::zeros();
    for (&source, &target) in source.iter().zip(target) {
        let a = project(&source_normal, source)?;
        let b = project(&target_normal, target)?;
        let xrow = SVector::<f64, 8>::from_row_slice(&[
            a.x,
            a.y,
            1.0,
            0.0,
            0.0,
            0.0,
            -b.x * a.x,
            -b.x * a.y,
        ]);
        let yrow = SVector::<f64, 8>::from_row_slice(&[
            0.0,
            0.0,
            0.0,
            a.x,
            a.y,
            1.0,
            -b.y * a.x,
            -b.y * a.y,
        ]);
        normal += xrow * xrow.transpose() + yrow * yrow.transpose();
        values += xrow * b.x + yrow * b.y;
    }
    let parameters = normal.lu().solve(&values)?;
    let normalized = Matrix3::new(
        parameters[0],
        parameters[1],
        parameters[2],
        parameters[3],
        parameters[4],
        parameters[5],
        parameters[6],
        parameters[7],
        1.0,
    );
    let mut transform = target_normal.try_inverse()? * normalized * source_normal;
    let scale = transform[(2, 2)];
    if scale.abs() < 1e-12 {
        return None;
    }
    transform /= scale;
    (transform.iter().all(|n| n.is_finite()) && transform.determinant().abs() > 1e-10)
        .then_some(transform)
}

fn normalization(points: &[Point]) -> Option<Homography> {
    let count = points.len() as f64;
    let x = points.iter().map(|p| p.x).sum::<f64>() / count;
    let y = points.iter().map(|p| p.y).sum::<f64>() / count;
    let distance = points.iter().map(|p| (p.x - x).hypot(p.y - y)).sum::<f64>() / count;
    if !distance.is_finite() || distance < 1e-9 {
        return None;
    }
    let scale = 2.0f64.sqrt() / distance;
    Some(Matrix3::new(
        scale,
        0.0,
        -scale * x,
        0.0,
        scale,
        -scale * y,
        0.0,
        0.0,
        1.0,
    ))
}

fn convex(quad: Quad) -> bool {
    if !quad.valid() || area(quad) < 1e-6 {
        return false;
    }
    let sign = cross(quad.0[0], quad.0[1], quad.0[2]).signum();
    (0..4).all(|i| {
        let turn = cross(quad.0[i], quad.0[(i + 1) % 4], quad.0[(i + 2) % 4]);
        turn.abs() > 1e-9 && turn.signum() == sign
    })
}

fn cross(a: Point, b: Point, c: Point) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

fn signed_area(quad: Quad) -> f64 {
    (0..4)
        .map(|i| {
            let (a, b) = (quad.0[i], quad.0[(i + 1) % 4]);
            a.x * b.y - a.y * b.x
        })
        .sum::<f64>()
        / 2.0
}

#[cfg(test)]
#[path = "tests/geometry.rs"]
mod tests;
