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
use serde_json::{Value, json};

use crate::work_dir::WorkDir;

/// The binaries a worker runs in, one per native GPU runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binary {
    /// `tbd-subtitles`: ONNX Runtime, FFmpeg and the `claude` CLI.
    Main,
    /// `tbd-subtitles-ggml`: Whisper through CrispASR.
    Ggml,
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
        StepName::Vad | StepName::DiffSheet | StepName::Cues | StepName::Qc | StepName::Output => {
            Placement::InProcess
        }
        StepName::AsrWhisper | StepName::RedecodeWhisper => Placement::Worker(Binary::Ggml),
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
    )
}

/// Whether the step's worker loads ONNX Runtime and so needs the runtime's environment: every
/// GPU step, and the review step, which runs Parakeet-CTC on the CPU.
pub fn loads_onnx_runtime(step: StepName) -> bool {
    uses_gpu(step) || step == StepName::Review
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
        ],
        Output => &[Cues],
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
    // Its findings name the utterance they are about, and the owner's corrections settle them;
    // a Fix It change the owner has not checked has its words checked again.
    (StepName::Qc, 4),
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
        Output => json!({ "output_format": settings.output_format }),
        _ => Value::Null,
    }
}

/// How long a step may run before it is killed.
pub fn timeout(step: StepName) -> Duration {
    let minutes = match step {
        StepName::Separation => 180,
        StepName::AsrWhisper | StepName::Adjudicate | StepName::SoundCues => 120,
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
        Qc => vec![work.qc()],
        Output => vec![
            work.output_record(),
            stages::output::subtitle_path(video, format),
        ],
    }
}

#[cfg(test)]
#[path = "tests/graph.rs"]
mod tests;
