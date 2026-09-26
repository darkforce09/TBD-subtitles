//! The models a job needs: one list, read by the tasks that load them and by the window that
//! downloads the missing ones before a job starts.
//!
//! **Role:** name each step's model folder (and file, where a folder holds several), and list the
//! model folders the job's settings need.
//!
//! **Position:** used by `tasks` to open models and by the app to check and fetch them through
//! `inference::model_store`.
//!
//! **Signals and state:** `missing` reads the models folder; nothing else touches the disk.
//!
//! **Invariants:** every folder `required` names is in the model store's manifest, so it can be
//! downloaded with a pinned checksum.

use std::path::{Path, PathBuf};

use inference::onnx::{ced, parakeet_ctc, parakeet_tdt};
use job_model::job::{JobSettings, Separator, WhisperModel};

use crate::error::{Context, Result};

/// The default models folder: `~/.local/share/tbd-subtitles/models`.
pub fn default_dir() -> Result<PathBuf> {
    inference::model_store::models_dir().context("cannot find the models folder")
}

/// The separation model's folder and file.
pub fn separator_model(separator: Separator) -> (&'static str, &'static str) {
    match separator {
        Separator::Roformer => (
            "mel-band-roformer-vocals",
            "syhft_core_folded_fp16_webgpu.onnx",
        ),
        Separator::MdxNet => ("mdx-net-voc-ft", "UVR-MDX-NET-Voc_FT.onnx"),
    }
}

/// Whisper's model folder and file.
pub fn whisper_model(model: WhisperModel) -> (&'static str, &'static str) {
    match model {
        WhisperModel::LargeV3 => ("whisper-large-v3", "ggml-large-v3.bin"),
        WhisperModel::LargeV3Turbo => ("whisper-large-v3-turbo", "ggml-large-v3-turbo-q8_0.bin"),
    }
}

/// The model folders a job with `settings` loads, in the order its steps load them.
pub fn required(settings: &JobSettings) -> Vec<&'static str> {
    vec![
        separator_model(settings.separator).0,
        parakeet_tdt::MODEL,
        whisper_model(settings.whisper).0,
        ced::MODEL,
        parakeet_ctc::MODEL,
    ]
}

/// The required model folders not complete in `models`.
pub fn missing(models: &Path, settings: &JobSettings) -> Vec<&'static str> {
    required(settings)
        .into_iter()
        .filter(|model| !inference::model_store::is_complete(models, model))
        .collect()
}

#[cfg(test)]
#[path = "tests/models.rs"]
mod tests;
