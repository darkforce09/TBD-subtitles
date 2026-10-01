//! Local detection and reading of writing in video frames.
//!
//! **Role:** expose PP-OCRv5 detection and recognition with a manga-ocr second reading, and the
//! detector pool that screens and confirms padded full-resolution frames.
//! **Position:** inference backend used inside the isolated visual GPU workers.
//! **Signals and state:** bounded ONNX predictors and a lazily opened manga reader. The mobile
//! detector screens batches of equal-sized frames at the 0.3 box score; the server detector
//! inspects single full-resolution frames at 0.5.
//! **Invariants:** CUDA registration fails explicitly; uncertain readings retain low confidence;
//! a screen returns one region list per image in input order; models are already exported and
//! downloaded through the pinned model store.

pub mod detector_pool;
mod manga;
pub mod pool;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use image::RgbImage;
use job_model::onscreen::{Point, Quad};
use oar_ocr::core::config::OrtSessionConfig;
use oar_ocr::domain::tasks::Detection;
use oar_ocr::predictors::{TextDetectionPredictor, TextRecognitionPredictor};

pub use detector_pool::{DetectorPool, PoolOptions, SearchMode};
pub use manga::MangaReader;

pub type OcrError = Box<dyn std::error::Error + Send + Sync>;

/// The box score a region needs to count in a screen; the predictor drops weaker boxes.
const SCREEN_SCORE: f64 = 0.45;
/// The box score a region needs in the exact single-frame pass.
const DETECT_SCORE: f64 = 0.5;

/// Text region detection on whole frames: a cheap presence screen and an exact pass.
pub trait TextDetection {
    /// Every region scoring at least 0.45 on each image, one list per image in input order, from
    /// one predictor call.
    fn screen_batch(&mut self, images: &[RgbImage]) -> Result<Vec<Vec<(Quad, f64)>>, OcrError>;
    /// Regions scoring at least 0.5 on one image.
    fn detect(&mut self, image: &RgbImage) -> Result<Vec<(Quad, f64)>, OcrError>;
}

/// The two PP-OCRv5 detectors of the visual detection worker: the mobile export screens many
/// small proxies cheaply, the server export confirms geometry on full-resolution stills.
pub struct OcrDetector {
    screen: TextDetectionPredictor,
    exact: TextDetectionPredictor,
}

impl OcrDetector {
    pub fn open(models_root: &Path) -> Result<Self, OcrError> {
        strict_cuda_environment()?;
        let screen = detector(&models_root.join("pp-ocrv5/det_mobile.onnx"), SCREEN_SCORE)?;
        let exact = detector(&models_root.join("pp-ocrv5/det.onnx"), DETECT_SCORE)?;
        Ok(Self { screen, exact })
    }
}

/// A detection predictor over `model` that keeps boxes scoring at least `box_score`.
fn detector(model: &Path, box_score: f64) -> Result<TextDetectionPredictor, OcrError> {
    Ok(TextDetectionPredictor::builder()
        .score_threshold(0.3)
        .box_threshold(box_score as f32)
        .unclip_ratio(1.5)
        .max_candidates(1000)
        .with_ort_config(OrtSessionConfig::new().with_intra_threads(4))
        .build(model)?)
}

impl TextDetection for OcrDetector {
    fn screen_batch(&mut self, images: &[RgbImage]) -> Result<Vec<Vec<(Quad, f64)>>, OcrError> {
        let Some(first) = images.first() else {
            return Ok(Vec::new());
        };
        for image in images {
            check_image(image)?;
            if image.dimensions() != first.dimensions() {
                return Err("screening batches need equal image sizes".into());
            }
        }
        let output = self.screen.predict(images.to_vec())?;
        if output.detections.len() != images.len() {
            return Err("OCR returned a different number of images".into());
        }
        let screened = output
            .detections
            .into_iter()
            .map(|detections| regions(detections, SCREEN_SCORE))
            .collect::<Result<Vec<_>, _>>()?;
        tracing::debug!(
            model = "PP-OCRv5 mobile detector",
            images = screened.len(),
            regions = screened.iter().map(Vec::len).sum::<usize>(),
            "OCR screening"
        );
        Ok(screened)
    }

    fn detect(&mut self, image: &RgbImage) -> Result<Vec<(Quad, f64)>, OcrError> {
        check_image(image)?;
        let output = self.exact.predict(vec![image.clone()])?;
        let detections = output
            .detections
            .into_iter()
            .next()
            .ok_or("OCR returned no image")?;
        let found = regions(detections, DETECT_SCORE)?;
        tracing::debug!(
            model = "PP-OCRv5 detector",
            regions = found.len(),
            "OCR detection"
        );
        Ok(found)
    }
}

/// The valid quadrilaterals scoring at least `min_score`, their scores clamped to 0..=1.
fn regions(detections: Vec<Detection>, min_score: f64) -> Result<Vec<(Quad, f64)>, OcrError> {
    let mut kept = Vec::with_capacity(detections.len());
    for detection in detections {
        if detection.bbox.points.len() != 4 {
            return Err("OCR detector returned a non-quadrilateral text region".into());
        }
        let quad = Quad(std::array::from_fn(|i| Point {
            x: f64::from(detection.bbox.points[i].x),
            y: f64::from(detection.bbox.points[i].y),
        }));
        let score = f64::from(detection.score);
        if quad.valid() && score.is_finite() && score >= min_score {
            kept.push((quad, score.clamp(0.0, 1.0)));
        }
    }
    Ok(kept)
}

/// A primary recognizer with an independent local reading for difficult crops.
pub struct OcrReader {
    predictor: TextRecognitionPredictor,
    manga: Option<MangaReader>,
    models_root: PathBuf,
}

impl OcrReader {
    pub fn open(models_root: &Path) -> Result<Self, OcrError> {
        strict_cuda_environment()?;
        let predictor = TextRecognitionPredictor::builder()
            .score_threshold(0.0)
            .dict_path(models_root.join("pp-ocrv5/dict.txt"))
            .with_ort_config(OrtSessionConfig::new().with_intra_threads(4))
            .build(models_root.join("pp-ocrv5/rec.onnx"))?;
        Ok(Self {
            predictor,
            manga: None,
            models_root: models_root.to_owned(),
        })
    }

    /// The PP-OCRv5 reading alone, with its confidence: Japanese and Latin script alike, never
    /// the Japanese-only second reader.
    pub fn read_primary(&mut self, image: &RgbImage) -> Result<(String, f64), OcrError> {
        check_image(image)?;
        let result = self.predictor.predict(vec![image.clone()])?;
        let text = result
            .texts
            .into_iter()
            .next()
            .ok_or("OCR returned no reading")?;
        let score = result
            .scores
            .first()
            .copied()
            .ok_or("OCR returned no reading confidence")?;
        let confidence = if score.is_finite() {
            f64::from(score).clamp(0.0, 1.0)
        } else {
            0.0
        };
        tracing::debug!(model = "PP-OCRv5 reader", text, confidence, "OCR reading");
        Ok((text, confidence))
    }

    pub fn read(&mut self, image: &RgbImage) -> Result<(String, f64), OcrError> {
        let (text, confidence) = self.read_primary(image)?;
        let vertical = f64::from(image.height()) > f64::from(image.width()) * 1.2;
        if !vertical && !text.is_empty() && confidence >= 0.88 {
            return Ok((text, confidence));
        }
        if self.manga.is_none() {
            self.manga = Some(MangaReader::open(&self.models_root)?);
        }
        let manga = self
            .manga
            .as_mut()
            .ok_or("Manga OCR reader is unavailable")?;
        let (alternative, alternative_confidence) = manga.read(image)?;
        if alternative.is_empty() {
            return Ok((text, confidence.min(0.79)));
        }
        if text.is_empty() || vertical {
            return Ok((alternative, alternative_confidence.min(0.79)));
        }
        if same_reading(&text, &alternative) {
            return Ok((text, confidence.max(alternative_confidence)));
        }
        // Agreement between independent readers matters more than incomparable score scales.
        if alternative_confidence > confidence {
            Ok((alternative, alternative_confidence.min(0.79)))
        } else {
            Ok((text, confidence.min(0.79)))
        }
    }
}

fn same_reading(left: &str, right: &str) -> bool {
    let normalize = |s: &str| {
        s.chars()
            .filter(|c| !c.is_whitespace())
            .map(|c| {
                let n = u32::from(c);
                if (0xff01..=0xff5e).contains(&n) {
                    char::from_u32(n - 0xfee0).unwrap_or(c)
                } else {
                    c
                }
            })
            .collect::<String>()
    };
    normalize(left) == normalize(right)
}

fn check_image(image: &RgbImage) -> Result<(), OcrError> {
    let pixels = u64::from(image.width()) * u64::from(image.height());
    if pixels == 0 || pixels > 33_554_432 {
        return Err("OCR needs a nonempty image containing at most 32 million pixels".into());
    }
    Ok(())
}

/// Inherit the worker's strict CUDA provider exactly once per session.
fn open_session(model: &Path) -> Result<ort::session::Session, OcrError> {
    strict_cuda_environment()?;
    Ok(ort::session::Session::builder()?
        .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
        .map_err(|error| crate::onnx::OnnxError::new("configuring OCR graph optimization", error))?
        .with_intra_threads(4)
        .map_err(|error| crate::onnx::OnnxError::new("configuring OCR inference threads", error))?
        .commit_from_file(model)
        .map_err(|error| {
            crate::onnx::OnnxError::new(format!("opening {}", model.display()), error)
        })?)
}

/// Commit the process's ONNX Runtime environment with the strict CUDA provider, once; an error
/// when another environment came first. A process that opens any ONNX session besides the OCR
/// ones calls this before them.
pub fn strict_cuda_environment() -> Result<(), OcrError> {
    static INITIALIZED: OnceLock<bool> = OnceLock::new();
    if *INITIALIZED.get_or_init(|| {
        use ort::ep::{ArenaExtendStrategy, CUDA, cuda::ConvAlgorithmSearch};
        ort::init()
            .with_execution_providers([CUDA::default()
                .with_memory_limit(3 * 1024 * 1024 * 1024)
                .with_arena_extend_strategy(ArenaExtendStrategy::SameAsRequested)
                .with_conv_algorithm_search(ConvAlgorithmSearch::Heuristic)
                .with_conv_max_workspace(false)
                .build()
                .error_on_failure()])
            .commit()
    }) {
        Ok(())
    } else {
        Err("OCR must start in its isolated worker before any other ONNX session".into())
    }
}
