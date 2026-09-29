//! Local detection and reading of writing in video frames.
//!
//! **Role:** expose PP-OCRv5 detection and recognition with a manga-ocr second reading.
//! **Position:** inference backend used inside the isolated visual GPU workers.
//! **Signals and state:** two bounded ONNX predictors and a lazily opened manga reader.
//! **Invariants:** CUDA registration fails explicitly; uncertain readings retain low confidence;
//! models are already exported and downloaded through the pinned model store.

mod manga;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use image::RgbImage;
use job_model::onscreen::{Point, Quad};
use oar_ocr::core::config::OrtSessionConfig;
use oar_ocr::predictors::{TextDetectionPredictor, TextRecognitionPredictor};

pub use manga::MangaReader;

pub type OcrError = Box<dyn std::error::Error + Send + Sync>;

/// A PP-OCRv5 detector loaded once for the visual detection worker.
pub struct OcrDetector {
    predictor: TextDetectionPredictor,
}

impl OcrDetector {
    pub fn open(models_root: &Path) -> Result<Self, OcrError> {
        strict_cuda_environment()?;
        let predictor = TextDetectionPredictor::builder()
            .score_threshold(0.3)
            .box_threshold(0.5)
            .unclip_ratio(1.5)
            .max_candidates(1000)
            .with_ort_config(OrtSessionConfig::new().with_intra_threads(4))
            .build(models_root.join("pp-ocrv5/det.onnx"))?;
        Ok(Self { predictor })
    }

    pub fn detect(&mut self, image: &RgbImage) -> Result<Vec<(Quad, f64)>, OcrError> {
        check_image(image)?;
        let output = self.predictor.predict(vec![image.clone()])?;
        let detections = output
            .detections
            .into_iter()
            .next()
            .ok_or("OCR returned no image")?;
        let mut regions = Vec::with_capacity(detections.len());
        for detection in detections {
            if detection.bbox.points.len() != 4 {
                return Err("OCR detector returned a non-quadrilateral text region".into());
            }
            let quad = Quad(std::array::from_fn(|i| Point {
                x: f64::from(detection.bbox.points[i].x),
                y: f64::from(detection.bbox.points[i].y),
            }));
            if quad.valid() && detection.score.is_finite() {
                regions.push((quad, f64::from(detection.score).clamp(0.0, 1.0)));
            }
        }
        tracing::debug!(
            model = "PP-OCRv5 detector",
            regions = regions.len(),
            "OCR detection"
        );
        Ok(regions)
    }
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

    pub fn read(&mut self, image: &RgbImage) -> Result<(String, f64), OcrError> {
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

fn strict_cuda_environment() -> Result<(), OcrError> {
    static INITIALIZED: OnceLock<bool> = OnceLock::new();
    if *INITIALIZED.get_or_init(|| {
        use ort::ep::{ArenaExtendStrategy, CUDA, cuda::ConvAlgorithmSearch};
        ort::init()
            .with_execution_providers([CUDA::default()
                .with_memory_limit(2 * 1024 * 1024 * 1024)
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
