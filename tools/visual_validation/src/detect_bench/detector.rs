//! The two ways the detection benchmark runs a PP-OCRv5 detector over a batch of frames.
//!
//! **Role:** the stock oar-ocr predictor at a chosen resize limit (a 1920-wide frame stretched to
//! 1088 rows at limit 1920), and a padded path that copies each frame into a reused buffer whose
//! height is the next multiple of 32, normalizes the batch, runs the DB model once and
//! post-processes each image on its own rayon task, clipping boxes back into the frame.
//! **Position:** opened per measured row by `runs.rs`; each instance owns one ONNX Runtime
//! session on CUDA with its own arena limit.
//! **Signals and state:** a session, and for the padded path its batch buffers.
//! **Invariants:** a screen returns one region list per frame in input order; the padded rows are
//! black and no box reaches below the frame's last row.

use std::path::Path;

use image::RgbImage;
use ndarray::s;
use oar_ocr::core::config::{OrtExecutionProvider, OrtSessionConfig};
use oar_ocr::domain::tasks::TextDetectionConfig;
use oar_ocr::models::detection::{DBModel, DBModelBuilder, DBPostprocessConfig};
use oar_ocr::predictors::TextDetectionPredictor;
use oar_ocr::processors::{
    BoxType, DBPostProcess, ImageScaleInfo, LimitType, NormalizeImage, ScoreMode, TensorLayout,
    types::ColorOrder,
};
use rayon::prelude::*;

/// A text region's four corners in frame pixels, and its box score.
pub type Quad = [(f32, f32); 4];
/// Every region of every frame of a batch, in input order.
pub type Screened = Vec<Vec<(Quad, f32)>>;

/// The binarization threshold every detector here uses, as production does.
const PIXEL_SCORE: f32 = 0.3;
const UNCLIP_RATIO: f32 = 1.5;
const MAX_CANDIDATES: usize = 1000;

/// A detector instance that screens batches of frames.
pub trait Screen: Send {
    fn screen(&mut self, frames: &[&RgbImage]) -> Result<Screened, String>;
}

/// How a detector instance is built.
#[derive(Debug, Clone)]
pub struct Settings<'a> {
    pub model: &'a Path,
    /// The longest side the stock predictor resizes to; its default (960) when `None`.
    pub limit_side: Option<u32>,
    /// The box score a region needs to be kept.
    pub box_score: f32,
    /// The session's CUDA arena limit, in MiB.
    pub memory_limit_mib: u64,
}

/// The production session options, with CUDA and this instance's arena limit.
fn session_config(memory_limit_mib: u64) -> OrtSessionConfig {
    let mut config = OrtSessionConfig::new().with_intra_threads(4);
    config.execution_providers = Some(vec![OrtExecutionProvider::CUDA {
        device_id: Some(0),
        gpu_mem_limit: Some((memory_limit_mib as usize) << 20),
        arena_extend_strategy: Some("SameAsRequested".into()),
        cudnn_conv_algo_search: Some("Heuristic".into()),
        cudnn_conv_use_max_workspace: Some(false),
    }]);
    config
}

/// The oar-ocr text detection predictor.
pub struct Stock(TextDetectionPredictor);

impl Stock {
    pub fn open(settings: &Settings<'_>) -> Result<Stock, String> {
        let config = TextDetectionConfig {
            score_threshold: PIXEL_SCORE,
            box_threshold: settings.box_score,
            unclip_ratio: UNCLIP_RATIO,
            max_candidates: MAX_CANDIDATES,
            limit_side_len: settings.limit_side,
            limit_type: settings.limit_side.map(|_| LimitType::Max),
            max_side_len: None,
        };
        TextDetectionPredictor::builder()
            .with_config(config)
            .with_ort_config(session_config(settings.memory_limit_mib))
            .build(settings.model)
            .map(Stock)
            .map_err(|error| format!("{error}"))
    }
}

impl Screen for Stock {
    fn screen(&mut self, frames: &[&RgbImage]) -> Result<Screened, String> {
        let owned: Vec<RgbImage> = frames.iter().map(|frame| (*frame).clone()).collect();
        let output = self.0.predict(owned).map_err(|error| format!("{error}"))?;
        Ok(output
            .detections
            .into_iter()
            .map(|detections| {
                detections
                    .into_iter()
                    .filter_map(|d| Some((quad(&d.bbox.points)?, d.score)))
                    .collect()
            })
            .collect())
    }
}

/// The four corners of a box with exactly four points.
fn quad(points: &[oar_ocr::processors::Point]) -> Option<Quad> {
    (points.len() == 4).then(|| std::array::from_fn(|i| (points[i].x, points[i].y)))
}

/// The DB model fed padded full-resolution frames.
pub struct Padded {
    model: DBModel,
    post: DBPostProcess,
    normalizer: NormalizeImage,
    buffers: Vec<RgbImage>,
    frame: (u32, u32),
}

impl Padded {
    /// A padded detector for frames of `frame` size.
    pub fn open(settings: &Settings<'_>, frame: (u32, u32)) -> Result<Padded, String> {
        let post_config = DBPostprocessConfig {
            score_threshold: PIXEL_SCORE,
            box_threshold: settings.box_score,
            unclip_ratio: UNCLIP_RATIO,
            max_candidates: MAX_CANDIDATES,
            use_dilation: false,
            score_mode: ScoreMode::Fast,
            box_type: BoxType::Quad,
        };
        let model = DBModelBuilder::new()
            .postprocess_config(post_config)
            .with_ort_config(session_config(settings.memory_limit_mib))
            .build(settings.model)
            .map_err(|error| format!("{error}"))?;
        let post = DBPostProcess::new(
            Some(PIXEL_SCORE),
            Some(settings.box_score),
            Some(MAX_CANDIDATES),
            Some(UNCLIP_RATIO),
            Some(false),
            Some(ScoreMode::Fast),
            Some(BoxType::Quad),
        );
        // The DB model's own normalization: ImageNet statistics in BGR order, CHW.
        let normalizer = NormalizeImage::with_color_order(
            Some(1.0 / 255.0),
            Some(vec![0.485, 0.456, 0.406]),
            Some(vec![0.229, 0.224, 0.225]),
            Some(TensorLayout::CHW),
            Some(ColorOrder::BGR),
        )
        .map_err(|error| format!("{error}"))?;
        Ok(Padded {
            model,
            post,
            normalizer,
            buffers: Vec::new(),
            frame,
        })
    }
}

impl Screen for Padded {
    fn screen(&mut self, frames: &[&RgbImage]) -> Result<Screened, String> {
        let (width, height) = self.frame;
        let padded = padded_height(height);
        while self.buffers.len() < frames.len() {
            self.buffers.push(RgbImage::new(width, padded));
        }
        for (frame, buffer) in frames.iter().zip(&mut self.buffers) {
            if frame.dimensions() != (width, height) {
                return Err("the padded path needs frames of its own size".into());
            }
            let bytes: &mut [u8] = buffer;
            bytes[..frame.as_raw().len()].copy_from_slice(frame.as_raw());
        }
        let refs: Vec<&RgbImage> = self.buffers[..frames.len()].iter().collect();
        let tensor = self
            .normalizer
            .normalize_batch_refs(&refs)
            .map_err(|error| format!("{error}"))?;
        let predictions = self
            .model
            .infer(&tensor)
            .map_err(|error| format!("{error}"))?;
        let post = &self.post;
        let shape = ImageScaleInfo::new(padded as f32, width as f32, 1.0, 1.0);
        Ok((0..frames.len())
            .into_par_iter()
            .map(|index| {
                let one = predictions
                    .slice(s![index..index + 1, .., .., ..])
                    .to_owned();
                let (boxes, scores) = post.apply(&one, vec![shape], None);
                boxes
                    .into_iter()
                    .next()
                    .unwrap_or_default()
                    .into_iter()
                    .zip(scores.into_iter().next().unwrap_or_default())
                    .filter_map(|(found, score)| {
                        Some((clip_quad(quad(&found.points)?, height), score))
                    })
                    .collect()
            })
            .collect())
    }
}

/// `height` rounded up to the next multiple of 32, as the DB model's input needs.
pub fn padded_height(height: u32) -> u32 {
    height.div_ceil(32) * 32
}

/// `corners`, found in a frame padded below its last row, with every corner moved into the
/// `height`-row frame.
pub fn clip_quad(corners: Quad, height: u32) -> Quad {
    let bottom = height as f32;
    corners.map(|(x, y)| (x, y.clamp(0.0, bottom)))
}

#[cfg(test)]
#[path = "tests/detector.rs"]
mod tests;
