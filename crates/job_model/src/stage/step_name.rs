//! Every step of the pipeline, in run order: the unit the job runner runs, resumes and times.
//!
//! **Role:** the one list of steps. A stage is one or more steps (speech recognition has one per
//! engine; adjudication has its first pass, the re-decode of unsure spans, the second pass and
//! the choice of sound cues); each step has one output, one fingerprint and one timing row.
//!
//! **Position:** used by `pipeline` (graph, resume, workers), by the `worker` subcommands of both
//! app binaries, and by the job report; depends on `serde` and [`StageName`].
//!
//! **Signals and state:** none.
//!
//! **Invariants:** [`StepName::ALL`] lists every variant exactly once, in run order; steps of one
//! stage are contiguous and the stages follow [`StageName::ALL`]; each name parses back to its
//! own variant.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::StageName;

/// One step of the pipeline.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[rkyv(compare(PartialEq), derive(Debug, PartialEq, Eq, PartialOrd, Ord))]
pub enum StepName {
    /// ffprobe the video and stream the mix to 16 kHz mono.
    ProbeDecode,
    /// Scan the video for shot changes, alongside the GPU steps.
    ShotScan,
    /// Split the mix into the vocal and background stems.
    Separation,
    /// Find speech in the vocal stem and plan the chunks.
    Vad,
    /// Parakeet, the backbone engine, over the chunks of the mix.
    AsrParakeet,
    /// Whisper, the second engine, over the same chunks.
    AsrWhisper,
    /// Line the engines up into the sheet the language model reads.
    DiffSheet,
    /// Tag sound events on both stems.
    SoundEvents,
    /// The language model's first pass over the whole sheet.
    Adjudicate,
    /// Parakeet again over the unsure utterances, on the vocal stem.
    RedecodeParakeet,
    /// Whisper again over the unsure utterances, on the vocal stem.
    RedecodeWhisper,
    /// The language model's second pass over the unsure utterances only.
    Readjudicate,
    /// The language model chooses and words the sound cues.
    SoundCues,
    /// Force-align the final text against the vocal stem.
    Alignment,
    /// Time again the lines the owner corrected, each alone, and keep every other line's times.
    Review,
    /// Lay the words and sound cues out as subtitle cues.
    Cues,
    /// Locate visible text regions in the video frames.
    TextDetect,
    /// Read Japanese text from representative crops.
    TextRead,
    /// Follow each visible text occurrence through its scene.
    TextTrack,
    /// Translate recognized text with its scene context.
    TextTranslate,
    /// Apply the owner's corrections to on-screen text.
    TextReview,
    /// Measure the strokes of replaceable writing and the background runs behind it.
    TextMask,
    /// Fill the erased strokes from the surrounding picture.
    TextInpaint,
    /// Draw the English lettering onto the filled background.
    TextCompose,
    /// Read the finished picture back with a local OCR and approve each lettered replacement.
    TextVerify,
    /// Lay out tracked English translations as ASS events.
    TextTypeset,
    /// Check the cues and write the report.
    Qc,
    /// Write the subtitle file beside the video.
    Output,
    /// Write the video with its writing replaced in English beside the source.
    LocalizedVideo,
}

impl StepName {
    /// Every step, in the order the job runner runs them.
    pub const ALL: [StepName; 29] = [
        StepName::ProbeDecode,
        StepName::ShotScan,
        StepName::Separation,
        StepName::Vad,
        StepName::AsrParakeet,
        StepName::AsrWhisper,
        StepName::DiffSheet,
        StepName::SoundEvents,
        StepName::Adjudicate,
        StepName::RedecodeParakeet,
        StepName::RedecodeWhisper,
        StepName::Readjudicate,
        StepName::SoundCues,
        StepName::Alignment,
        StepName::Review,
        StepName::Cues,
        StepName::TextDetect,
        StepName::TextRead,
        StepName::TextTrack,
        StepName::TextTranslate,
        StepName::TextReview,
        StepName::TextMask,
        StepName::TextInpaint,
        StepName::TextCompose,
        StepName::TextVerify,
        StepName::TextTypeset,
        StepName::Qc,
        StepName::Output,
        StepName::LocalizedVideo,
    ];

    /// The name on the command line, in file names and in JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            StepName::ProbeDecode => "probe_decode",
            StepName::ShotScan => "shot_scan",
            StepName::Separation => "separation",
            StepName::Vad => "vad",
            StepName::AsrParakeet => "asr_parakeet",
            StepName::AsrWhisper => "asr_whisper",
            StepName::DiffSheet => "diff_sheet",
            StepName::SoundEvents => "sound_events",
            StepName::Adjudicate => "adjudicate",
            StepName::RedecodeParakeet => "redecode_parakeet",
            StepName::RedecodeWhisper => "redecode_whisper",
            StepName::Readjudicate => "readjudicate",
            StepName::SoundCues => "sound_cues",
            StepName::Alignment => "alignment",
            StepName::Review => "review",
            StepName::Cues => "cues",
            StepName::TextDetect => "text_detect",
            StepName::TextRead => "text_read",
            StepName::TextTrack => "text_track",
            StepName::TextTranslate => "text_translate",
            StepName::TextReview => "text_review",
            StepName::TextMask => "text_mask",
            StepName::TextInpaint => "text_inpaint",
            StepName::TextCompose => "text_compose",
            StepName::TextVerify => "text_verify",
            StepName::TextTypeset => "text_typeset",
            StepName::Qc => "qc",
            StepName::Output => "output",
            StepName::LocalizedVideo => "localized_video",
        }
    }

    /// The stage the step belongs to.
    pub fn stage(self) -> StageName {
        match self {
            StepName::ProbeDecode | StepName::ShotScan => StageName::ProbeDecode,
            StepName::Separation => StageName::Separation,
            StepName::Vad => StageName::Vad,
            StepName::AsrParakeet | StepName::AsrWhisper => StageName::Asr,
            StepName::DiffSheet => StageName::DiffSheet,
            StepName::SoundEvents => StageName::SoundEvents,
            StepName::Adjudicate
            | StepName::RedecodeParakeet
            | StepName::RedecodeWhisper
            | StepName::Readjudicate
            | StepName::SoundCues => StageName::Adjudication,
            StepName::Alignment | StepName::Review => StageName::Alignment,
            StepName::Cues => StageName::Cues,
            StepName::TextDetect
            | StepName::TextRead
            | StepName::TextTrack
            | StepName::TextTranslate
            | StepName::TextReview
            | StepName::TextMask
            | StepName::TextInpaint
            | StepName::TextCompose
            | StepName::TextVerify
            | StepName::TextTypeset => StageName::OnscreenText,
            StepName::Qc => StageName::Qc,
            StepName::Output => StageName::Output,
            StepName::LocalizedVideo => StageName::LocalizedVideo,
        }
    }
}

impl fmt::Display for StepName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A step name that names no step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownStep(pub String);

impl fmt::Display for UnknownStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}` is not a step", self.0)
    }
}

impl std::error::Error for UnknownStep {}

impl FromStr for StepName {
    type Err = UnknownStep;

    fn from_str(text: &str) -> Result<StepName, UnknownStep> {
        StepName::ALL
            .into_iter()
            .find(|step| step.as_str() == text)
            .ok_or_else(|| UnknownStep(text.to_string()))
    }
}

#[cfg(test)]
#[path = "tests/step_name.rs"]
mod tests;
