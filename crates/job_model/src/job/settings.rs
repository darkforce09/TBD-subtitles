//! The settings a job runs with: which models, which glossary, which audio track, the tuning of
//! the cue and language-model steps, and the subtitle file's format.
//!
//! **Role:** name every choice that changes a job's output, with the measured defaults.
//!
//! **Position:** built by the app from the owner's settings and the command line; stored in
//! `job.json`; each step's fingerprint covers the part it reads (`pipeline::graph::settings`).
//!
//! **Signals and state:** none; plain data.
//!
//! **Invariants:** the JSON names stay stable, and a field added later carries a serde default, so
//! an older `job.json` still parses and its finished steps stay valid.

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

/// The subtitle file format the output step writes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    /// SubRip: the default, what VLC and every player read.
    #[default]
    Srt,
    /// WebVTT.
    Vtt,
    /// Advanced SubStation Alpha.
    Ass,
}

impl OutputFormat {
    pub const ALL: [OutputFormat; 3] = [OutputFormat::Srt, OutputFormat::Vtt, OutputFormat::Ass];

    /// The file extension, without the dot.
    pub fn extension(self) -> &'static str {
        match self {
            OutputFormat::Srt => "srt",
            OutputFormat::Vtt => "vtt",
            OutputFormat::Ass => "ass",
        }
    }
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
    /// The subtitle file written beside the video.
    #[serde(default)]
    pub output_format: OutputFormat,
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
            output_format: OutputFormat::Srt,
        }
    }
}
