//! Text regions from the detector's probability maps.
//!
//! **Role:** turn each image's probability map into quadrilaterals with oar-ocr's DB
//! post-processing, one rayon task per image, and keep the regions inside the real frame.
//!
//! **Position:** called by the pool's session threads with the maps a run left in its output
//! buffer, read in place.
//!
//! **Signals and state:** the DB post-processor and the score a region needs.
//!
//! **Invariants:** coordinates are frame pixels: the map has the padded input's size, so no
//! scaling applies, and every corner is clipped into the frame, which drops what lies only in
//! the padding; a region is kept when it is a valid quadrilateral with a finite score of at least
//! the minimum, its score clamped to 0..=1; one region list per frame, in input order.

use job_model::onscreen::{Point, Quad};
use ndarray::Array4;
use oar_ocr::processors::{BoxType, DBPostProcess, ImageScaleInfo, ScoreMode};
use rayon::prelude::*;

use super::batch::InputShape;
use crate::ocr::{DETECT_SCORE, OcrError, SCREEN_SCORE};

/// The probability a map pixel needs to count as text, as the stock predictor uses.
const PIXEL_SCORE: f32 = 0.3;
const UNCLIP_RATIO: f32 = 1.5;
const MAX_CANDIDATES: usize = 1000;

/// One image's regions: corners in frame pixels and the box score.
pub type Regions = Vec<(Quad, f64)>;

/// DB post-processing at one box score.
pub struct PostProcess {
    db: DBPostProcess,
    min_score: f64,
}

impl PostProcess {
    fn at(min_score: f64) -> PostProcess {
        PostProcess {
            db: DBPostProcess::new(
                Some(PIXEL_SCORE),
                Some(min_score as f32),
                Some(MAX_CANDIDATES),
                Some(UNCLIP_RATIO),
                Some(false),
                Some(ScoreMode::Fast),
                Some(BoxType::Quad),
            ),
            min_score,
        }
    }

    /// The mobile detector's screening: regions scoring at least 0.45.
    pub fn screening() -> PostProcess {
        PostProcess::at(SCREEN_SCORE)
    }

    /// The server detector's confirmation: regions scoring at least 0.5.
    pub fn confirming() -> PostProcess {
        PostProcess::at(DETECT_SCORE)
    }

    /// The regions of the first `frames` images of `maps`, `[batch, 1, height, width]` as
    /// `shape` gives it, in frames of `width` × `height` pixels.
    pub fn regions(
        &self,
        maps: &[f32],
        shape: InputShape,
        frames: usize,
        (width, height): (u32, u32),
    ) -> Result<Vec<Regions>, OcrError> {
        if maps.len() != shape.output_len() || frames > shape.batch {
            return Err(format!(
                "the detector answered with {} values; {} were expected",
                maps.len(),
                shape.output_len()
            )
            .into());
        }
        let plane = shape.plane();
        let scale = ImageScaleInfo::new(shape.height as f32, shape.width as f32, 1.0, 1.0);
        (0..frames)
            .into_par_iter()
            .map(|image| {
                let map = &maps[image * plane..(image + 1) * plane];
                let owned = Array4::from_shape_vec((1, 1, shape.height, shape.width), map.to_vec())
                    .map_err(|error| format!("probability map: {error}"))?;
                let (boxes, scores) = self.db.apply(&owned, vec![scale], None);
                let boxes = boxes.into_iter().next().unwrap_or_default();
                let scores = scores.into_iter().next().unwrap_or_default();
                let mut kept = Vec::with_capacity(boxes.len());
                for (found, score) in boxes.into_iter().zip(scores) {
                    if found.points.len() != 4 {
                        return Err("the detector returned a non-quadrilateral text region".into());
                    }
                    let corners = std::array::from_fn(|i| (found.points[i].x, found.points[i].y));
                    if let Some(region) = region(corners, score, self.min_score, width, height) {
                        kept.push(region);
                    }
                }
                Ok(kept)
            })
            .collect()
    }
}

/// `corners`, found in a frame padded right and below, clipped into the `width` × `height`
/// frame.
pub fn clip_corners(corners: [(f32, f32); 4], width: u32, height: u32) -> Quad {
    let (right, bottom) = (f64::from(width), f64::from(height));
    Quad(corners.map(|(x, y)| Point {
        x: f64::from(x).clamp(0.0, right),
        y: f64::from(y).clamp(0.0, bottom),
    }))
}

/// The clipped region and its clamped score, when it is a valid quadrilateral scoring at least
/// `min_score`.
pub fn region(
    corners: [(f32, f32); 4],
    score: f32,
    min_score: f64,
    width: u32,
    height: u32,
) -> Option<(Quad, f64)> {
    let quad = clip_corners(corners, width, height);
    let score = f64::from(score);
    (quad.valid() && score.is_finite() && score >= min_score).then(|| (quad, score.clamp(0.0, 1.0)))
}

#[cfg(test)]
#[path = "tests/regions.rs"]
mod tests;
