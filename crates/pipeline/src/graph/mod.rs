//! The step graph: what each step reads, where it runs, whether it needs the GPU, which of the
//! settings it depends on, and which files it leaves.
//!
//! **Role:** the one table the runner, the resume check and the workers consult; the order is
//! `StepName::ALL`.
//!
//! **Position:** used by `runner`, `resume` and `workers`.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** a step reads only steps before it; every GPU step runs in a worker; the
//! Whisper steps run in the ggml binary and nothing else does; a step's revision changes
//! whenever its code changes what it writes, so older outputs are not reused.

use std::path::{Path, PathBuf};
use std::time::Duration;

use job_model::StepName;
use job_model::job::{JobSettings, OutputFormat};
use job_model::onscreen::LocalizedVideoRecord;
use job_model::outputs::OutputRecord;
use serde_json::{Value, json};

use crate::work_dir::WorkDir;

/// The binaries a worker runs in, one per native GPU runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binary {
    /// `tbd-subtitles`: ONNX Runtime, FFmpeg and the `claude` CLI.
    Main,
    /// `tbd-subtitles-ggml`: Whisper through CrispASR.
    Ggml,
    /// `tbd-subtitles-llm`: the local language model through mistral.rs.
    LocalLlm,
}

/// Where a step runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Inside the job runner: pure work on files, no model, no child process.
    InProcess,
    /// In a worker process of the given binary.
    Worker(Binary),
}

pub fn placement(step: StepName) -> Placement {
    match step {
        StepName::Vad
        | StepName::DiffSheet
        | StepName::Cues
        | StepName::TextReview
        | StepName::Qc
        | StepName::Output => Placement::InProcess,
        StepName::AsrWhisper | StepName::RedecodeWhisper => Placement::Worker(Binary::Ggml),
        StepName::TextTranslate => Placement::Worker(Binary::LocalLlm),
        _ => Placement::Worker(Binary::Main),
    }
}

/// Whether the step loads a model onto the GPU.
pub fn uses_gpu(step: StepName) -> bool {
    matches!(
        step,
        StepName::Separation
            | StepName::AsrParakeet
            | StepName::AsrWhisper
            | StepName::SoundEvents
            | StepName::RedecodeParakeet
            | StepName::RedecodeWhisper
            | StepName::Alignment
            | StepName::TextDetect
            | StepName::TextRead
            | StepName::TextTranslate
            | StepName::TextInpaint
            | StepName::TextVerify
            | StepName::LocalizedVideo
    )
}

/// Whether the step loads ONNX Runtime; ggml and mistral.rs workers keep their runtimes separate.
pub fn loads_onnx_runtime(step: StepName) -> bool {
    matches!(
        step,
        StepName::Separation
            | StepName::AsrParakeet
            | StepName::SoundEvents
            | StepName::RedecodeParakeet
            | StepName::Alignment
            | StepName::Review
            | StepName::TextDetect
            | StepName::TextRead
            | StepName::TextInpaint
            | StepName::TextVerify
    )
}

/// Whether the step's worker needs the packaged CUDA runtime on its library path: ONNX Runtime
/// loads CUDA from it, and ggml's CUDA backend links `libcudart` and `libcublas` found only there.
pub fn needs_cuda_runtime(step: StepName) -> bool {
    loads_onnx_runtime(step) || placement(step) == Placement::Worker(Binary::Ggml)
}

/// Whether the step's fingerprint covers the owner's corrections.
pub fn reads_corrections(step: StepName) -> bool {
    step == StepName::Review
}

/// The steps whose outputs a step reads.
pub fn inputs(step: StepName) -> &'static [StepName] {
    use StepName::*;
    match step {
        ProbeDecode | ShotScan => &[],
        Separation => &[ProbeDecode],
        Vad => &[ProbeDecode, Separation],
        AsrParakeet | AsrWhisper => &[ProbeDecode, Vad],
        DiffSheet => &[AsrParakeet, AsrWhisper],
        SoundEvents => &[ProbeDecode, Separation],
        Adjudicate => &[DiffSheet],
        RedecodeParakeet | RedecodeWhisper => &[ProbeDecode, Separation, DiffSheet, Adjudicate],
        Readjudicate => &[DiffSheet, Adjudicate, RedecodeParakeet, RedecodeWhisper],
        SoundCues => &[AsrWhisper, DiffSheet, SoundEvents, Readjudicate],
        Alignment => &[
            ProbeDecode,
            Separation,
            AsrParakeet,
            AsrWhisper,
            DiffSheet,
            Readjudicate,
        ],
        Review => &[
            ProbeDecode,
            Separation,
            AsrParakeet,
            AsrWhisper,
            DiffSheet,
            Readjudicate,
            Alignment,
        ],
        Cues => &[ProbeDecode, ShotScan, SoundCues, Review],
        TextDetect => &[ProbeDecode, ShotScan],
        TextRead => &[TextDetect],
        TextTrack => &[ProbeDecode, ShotScan, TextRead],
        TextTranslate => &[TextTrack, Cues],
        TextReview => &[ShotScan, TextTranslate],
        TextMask => &[ProbeDecode, TextReview],
        TextInpaint => &[TextMask],
        TextCompose => &[TextReview, TextInpaint],
        TextVerify => &[ProbeDecode, TextReview, TextCompose],
        TextTypeset => &[TextReview],
        Qc => &[
            ProbeDecode,
            Vad,
            DiffSheet,
            RedecodeParakeet,
            RedecodeWhisper,
            Readjudicate,
            SoundCues,
            Review,
            Cues,
            TextTypeset,
        ],
        Output => &[Cues, TextTypeset, TextVerify],
        LocalizedVideo => &[ProbeDecode, TextVerify, Output],
    }
}

/// Steps whose code changed what they write, with their revision; every other step is at 1.
const REVISIONS: &[(StepName, u32)] = &[
    // An utterance only another engine heard at its start or end is aligned where that engine
    // heard it.
    (StepName::Alignment, 2),
    (StepName::Review, 2),
    // A short cue joins its speaker's line of a dashed neighbour, or starts earlier into free
    // time.
    (StepName::Cues, 3),
    // Findings include tracked text alongside the spoken lines and their corrections.
    (StepName::Qc, 5),
    // ASS output combines dialogue and tracked English text; a localized video's own file carries
    // dialogue and sound cues alone, moved above English lettered into the picture.
    // The lettered writing it keeps subtitles clear of is what the read-back check approved.
    (StepName::Output, 5),
    // Sampled screening with bisected boundaries and one keyframe per occurrence.
    (StepName::TextDetect, 4),
    (StepName::TextRead, 3),
    // Sampled geometry replaces per-frame optical flow.
    (StepName::TextTrack, 3),
    // Claude reads every keyframe first; the local model answers what it leaves.
    (StepName::TextTranslate, 7),
    // One occurrence per sign, with furigana folded into their line.
    (StepName::TextReview, 3),
    // Typesetting writes the combined file's events alone; the localized file has none.
    (StepName::TextTypeset, 4),
    // Masks, fills and lettering account for ruby, check each erase and make one replacement per
    // sign.
    (StepName::TextMask, 2),
    (StepName::TextCompose, 2),
    // Strokes left after the wider retry no longer decide; the read-back check does.
    (StepName::TextInpaint, 3),
    // A local OCR reads each finished replacement back before it reaches the localized video.
    (StepName::TextVerify, 1),
    (StepName::LocalizedVideo, 2),
];

/// The revision of a step's code; a change makes every earlier output of the step stale.
pub fn revision(step: StepName) -> u32 {
    REVISIONS
        .iter()
        .find(|(s, _)| *s == step)
        .map_or(1, |(_, r)| *r)
}

/// The part of the settings a step depends on; the video's identity is covered separately.
pub fn settings(step: StepName, settings: &JobSettings) -> Value {
    use StepName::*;
    match step {
        ProbeDecode => json!({ "audio_track": settings.audio_track }),
        Separation => json!({ "separator": settings.separator }),
        AsrWhisper | RedecodeWhisper => json!({ "whisper": settings.whisper }),
        Adjudicate | Readjudicate | SoundCues => json!({
            "glossary": settings.glossary,
            "llm_model": settings.llm_model,
        }),
        Cues => json!({ "cut_score": settings.cut_score }),
        TextDetect | TextRead | TextTrack | TextReview => {
            json!({ "enabled": settings.onscreen_text.enabled })
        }
        TextMask | TextInpaint | TextCompose | TextVerify | TextTypeset | LocalizedVideo => json!({
            "enabled": settings.onscreen_text.enabled,
            "localized_video": settings.onscreen_text.localized_video,
        }),
        // Named fields keep the localized-video switch out of the translation fingerprint.
        TextTranslate => json!({
            "glossary": settings.glossary,
            "llm_model": settings.llm_model,
            "onscreen_text": {
                "enabled": settings.onscreen_text.enabled,
                "claude_fallback": settings.onscreen_text.claude_fallback,
                "reference_folder": settings.onscreen_text.reference_folder,
            },
        }),
        Output => json!({
            "output_format": settings.effective_output_format(),
            "localized_video": settings.onscreen_text.enabled
                && settings.onscreen_text.localized_video,
        }),
        _ => Value::Null,
    }
}

/// How long a step may run before it is killed.
pub fn timeout(step: StepName) -> Duration {
    let minutes = match step {
        StepName::Separation => 180,
        StepName::AsrWhisper | StepName::Adjudicate | StepName::SoundCues => 120,
        StepName::TextDetect
        | StepName::TextRead
        | StepName::TextTrack
        | StepName::TextTranslate
        | StepName::TextMask
        | StepName::TextInpaint
        | StepName::TextCompose
        | StepName::TextVerify
        | StepName::LocalizedVideo => 360,
        _ => 60,
    };
    Duration::from_secs(minutes * 60)
}

/// The files a finished step leaves; the step is redone when one is missing.
pub fn outputs(step: StepName, work: &WorkDir, video: &Path, format: OutputFormat) -> Vec<PathBuf> {
    use StepName::*;
    match step {
        ProbeDecode => vec![work.probe(), work.mix()],
        ShotScan => vec![work.shots()],
        Separation => vec![work.vocals(), work.background()],
        Vad => vec![work.vad()],
        AsrParakeet => vec![work.asr("parakeet")],
        AsrWhisper => vec![work.asr("whisper")],
        DiffSheet => vec![work.sheet(), work.sheet_text()],
        SoundEvents => vec![work.sound_events()],
        Adjudicate => vec![work.first_pass()],
        RedecodeParakeet => vec![work.redecode("parakeet")],
        RedecodeWhisper => vec![work.redecode("whisper")],
        Readjudicate => vec![work.adjudicated()],
        SoundCues => vec![work.sound_cues()],
        Alignment => vec![work.aligned()],
        Review => vec![work.reviewed()],
        Cues => vec![work.cues(), work.dropped_sounds()],
        TextDetect | TextRead | TextTrack | TextTranslate | TextReview => vec![work.text(step)],
        TextMask | TextInpaint | TextCompose | TextVerify => vec![work.text(step)],
        TextTypeset => vec![work.text(step), work.text_ass()],
        Qc => vec![work.qc()],
        Output => vec![
            work.output_record(),
            stages::output::subtitle_path(video, format),
        ],
        LocalizedVideo => vec![work.text(step)],
    }
}

/// Check declared outputs, the representative crops and the keyframe stills required to resume
/// text reading and translation.
pub fn artifacts_valid(step: StepName, work: &WorkDir, video: &Path, format: OutputFormat) -> bool {
    if !outputs(step, work, video, format)
        .iter()
        .all(|path| path.exists())
    {
        return false;
    }
    match step {
        StepName::TextDetect => {}
        StepName::Output => {
            return recorded_file_present::<OutputRecord>(&work.output_record(), |r| {
                r.localized.clone()
            });
        }
        StepName::LocalizedVideo => {
            return recorded_file_present::<LocalizedVideoRecord>(&work.text(step), |r| {
                r.path.clone()
            });
        }
        _ => return true,
    }
    // Deserialize only crop paths, skipping large geometry arrays without retaining them.
    #[derive(serde::Deserialize)]
    struct DetectionArtifacts {
        occurrences: Vec<CropArtifacts>,
    }
    #[derive(serde::Deserialize)]
    struct CropArtifacts {
        crops: Vec<PathBuf>,
        #[serde(default)]
        keyframe: Option<KeyframeArtifact>,
    }
    #[derive(serde::Deserialize)]
    struct KeyframeArtifact {
        image: PathBuf,
    }
    let Ok(file) = std::fs::File::open(work.text(step)) else {
        return false;
    };
    let Ok(artifacts) =
        serde_json::from_reader::<_, DetectionArtifacts>(std::io::BufReader::new(file))
    else {
        return false;
    };
    let present = |path: &PathBuf| {
        path.components().all(|part| {
            matches!(
                part,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        }) && std::fs::metadata(work.root().join(path))
            .is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
    };
    artifacts.occurrences.iter().all(|item| {
        !item.crops.is_empty()
            && item.crops.iter().all(present)
            && item
                .keyframe
                .as_ref()
                .is_none_or(|keyframe| present(&keyframe.image))
    })
}

/// Whether the file a step's record names, if it names one, still exists.
fn recorded_file_present<T: serde::de::DeserializeOwned>(
    record: &Path,
    named: impl Fn(&T) -> Option<String>,
) -> bool {
    let Ok(text) = std::fs::read_to_string(record) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<T>(&text) else {
        return false;
    };
    named(&value).is_none_or(|path| Path::new(&path).is_file())
}

#[cfg(test)]
#[path = "tests/graph.rs"]
mod tests;
