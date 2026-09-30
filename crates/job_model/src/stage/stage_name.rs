//! Every stage of the pipeline, in run order, with the name used on the command line and in JSON.
//!
//! **Role:** the one list of stages: the job runner walks it in order, the `worker` subcommand
//! accepts the stages that run in a worker process, and the work directory names outputs by it.
//!
//! **Position:** used by `pipeline`, `stages` and the app's `worker` subcommand; depends on
//! `serde` only.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** [`StageName::ALL`] lists every variant exactly once, in run order; each name
//! parses back to its own variant; only the stages that load a GPU model or the language model run
//! in a worker process.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// One stage of the pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageName {
    /// ffprobe the streams, stream the audio, scan shot changes.
    ProbeDecode,
    /// Split the mix into a vocal stem and a background stem.
    Separation,
    /// Find speech in the vocal stem and plan the chunks every engine transcribes.
    Vad,
    /// Run each speech engine over the chunks.
    Asr,
    /// Align the engines' words and write the sheet the language model reads.
    DiffSheet,
    /// Let the language model settle each disagreement and choose sound cues.
    Adjudication,
    /// Force-align the final text against the vocal stem.
    Alignment,
    /// Detect sound events on the stems.
    SoundEvents,
    /// Lay the aligned words out as subtitle cues.
    Cues,
    /// Recognize, track, translate and typeset text visible in the video.
    OnscreenText,
    /// Check the cues against the layout and coverage rules.
    Qc,
    /// Write the subtitle file beside the video.
    Output,
    /// Write the video with its writing replaced in English beside the source.
    LocalizedVideo,
}

impl StageName {
    /// Every stage, in the order the job runner runs them.
    pub const ALL: [StageName; 13] = [
        StageName::ProbeDecode,
        StageName::Separation,
        StageName::Vad,
        StageName::Asr,
        StageName::DiffSheet,
        StageName::SoundEvents,
        StageName::Adjudication,
        StageName::Alignment,
        StageName::Cues,
        StageName::OnscreenText,
        StageName::Qc,
        StageName::Output,
        StageName::LocalizedVideo,
    ];

    /// The name on the command line, in file names and in JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            StageName::ProbeDecode => "probe_decode",
            StageName::Separation => "separation",
            StageName::Vad => "vad",
            StageName::Asr => "asr",
            StageName::DiffSheet => "diff_sheet",
            StageName::Adjudication => "adjudication",
            StageName::Alignment => "alignment",
            StageName::SoundEvents => "sound_events",
            StageName::Cues => "cues",
            StageName::OnscreenText => "onscreen_text",
            StageName::Qc => "qc",
            StageName::Output => "output",
            StageName::LocalizedVideo => "localized_video",
        }
    }

    /// Whether the stage loads a GPU model or the language model and so runs in its own worker
    /// process, which frees the memory and keeps native libraries apart when it exits.
    pub fn runs_in_worker(self) -> bool {
        matches!(
            self,
            StageName::Separation
                | StageName::Asr
                | StageName::Adjudication
                | StageName::Alignment
                | StageName::SoundEvents
                | StageName::OnscreenText
                | StageName::LocalizedVideo
        )
    }
}

impl fmt::Display for StageName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A stage name that names no stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownStage(pub String);

impl fmt::Display for UnknownStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}` is not a stage", self.0)
    }
}

impl std::error::Error for UnknownStage {}

impl FromStr for StageName {
    type Err = UnknownStage;

    fn from_str(text: &str) -> Result<StageName, UnknownStage> {
        StageName::ALL
            .into_iter()
            .find(|stage| stage.as_str() == text)
            .ok_or_else(|| UnknownStage(text.to_string()))
    }
}

#[cfg(test)]
#[path = "tests/stage_name.rs"]
mod tests;
