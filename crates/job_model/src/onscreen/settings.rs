//! Settings for the visible-text branch.
//!
//! **Role:** separate new-job defaults from compatibility defaults for saved jobs.
//! **Position:** consumed by job settings, GUI settings and step fingerprints.
//! **Signals and state:** plain serializable data.
//! **Invariants:** absent settings in an old job disable the branch and the localized video; new
//! jobs enable both; how frames are decoded never changes what a step writes, so
//! `hardware_decode` is left out of every fingerprint, while the detector engine and the localized
//! video's encoder are covered by the steps that use them.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The ONNX Runtime execution provider the on-screen text detectors run on.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[rkyv(compare(PartialEq), derive(Debug, PartialEq, Eq))]
pub enum DetectorEngine {
    /// The CUDA provider: cuDNN convolutions on the session's own stream.
    #[default]
    Cuda,
    /// The TensorRT provider: an FP16 engine built and cached per GPU, driver and model.
    TensorRt,
}

impl DetectorEngine {
    pub const ALL: [DetectorEngine; 2] = [DetectorEngine::TensorRt, DetectorEngine::Cuda];

    /// The name the Settings window shows.
    pub fn label(self) -> &'static str {
        match self {
            DetectorEngine::Cuda => "CUDA",
            DetectorEngine::TensorRt => "TensorRT",
        }
    }
}

/// The encoder that re-encodes the localized video's changed segments.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[rkyv(compare(PartialEq), derive(Debug, PartialEq, Eq))]
pub enum LocalizedEncoder {
    /// libx264 on the CPU, built to be joined to the source's own stream.
    #[default]
    X264,
    /// NVENC on the GPU.
    Nvenc,
}

impl LocalizedEncoder {
    pub const ALL: [LocalizedEncoder; 2] = [LocalizedEncoder::X264, LocalizedEncoder::Nvenc];

    /// The name the Settings window shows.
    pub fn label(self) -> &'static str {
        match self {
            LocalizedEncoder::X264 => "x264 (CPU)",
            LocalizedEncoder::Nvenc => "NVENC (GPU)",
        }
    }
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(default)]
pub struct TextSettings {
    pub enabled: bool,
    pub claude_fallback: bool,
    #[rkyv(with = rkyv::with::Map<rkyv::with::AsString>)]
    pub reference_folder: Option<PathBuf>,
    /// Erase replaceable writing and draw its English into `<video>.localized.mkv`.
    pub localized_video: bool,
    /// Decode the video on the GPU (NVDEC) instead of the CPU; the frames are the same.
    pub hardware_decode: bool,
    /// The execution provider the detectors run on.
    pub detector_engine: DetectorEngine,
    /// The encoder of the localized video's changed segments.
    pub localized_encoder: LocalizedEncoder,
}

impl Default for TextSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            claude_fallback: true,
            reference_folder: None,
            localized_video: false,
            hardware_decode: false,
            detector_engine: DetectorEngine::default(),
            localized_encoder: LocalizedEncoder::default(),
        }
    }
}

impl TextSettings {
    pub fn new_job() -> Self {
        Self {
            enabled: true,
            localized_video: true,
            ..Self::default()
        }
    }
}
