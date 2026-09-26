//! The settings a job runs with: which models, which glossary, which audio track, and the tuning
//! of the cue and language-model steps.

use serde::{Deserialize, Serialize};

/// The vocal-separation model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Separator {
    /// Mel-Band RoFormer: the default, the cleaner vocal stem.
    Roformer,
    /// UVR MDX-Net Voc_FT: the fast mode.
    MdxNet,
}

/// The second speech engine's model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WhisperModel {
    /// Whisper large-v3: the default.
    LargeV3,
    /// Whisper large-v3-turbo, 8-bit: the fallback when a video runs over budget.
    LargeV3Turbo,
}

/// Everything that changes a job's output. A step's fingerprint covers the settings it reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobSettings {
    pub separator: Separator,
    pub whisper: WhisperModel,
    /// The audio track's position among the audio streams (`0:a:<n>`); `None` picks the English
    /// track.
    pub audio_track: Option<u32>,
    /// Names and terms the language model spells right.
    pub glossary: Vec<String>,
    /// The `claude` model name the language-model steps ask.
    pub llm_model: String,
    /// How many `claude` processes run at once.
    pub llm_processes: usize,
    /// The lowest scdet score that counts as a shot cut for cue timing.
    pub cut_score: f64,
}

impl JobSettings {
    /// The defaults for a video, with the given glossary.
    pub fn with_glossary(glossary: Vec<String>) -> JobSettings {
        JobSettings {
            separator: Separator::Roformer,
            whisper: WhisperModel::LargeV3,
            audio_track: None,
            glossary,
            llm_model: "sonnet".to_string(),
            llm_processes: 8,
            cut_score: 20.0,
        }
    }
}
