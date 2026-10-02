//! The step graph: what each step reads, where it runs, whether it needs the GPU, which of the
//! settings it depends on, which stored values it reads, which steps depend on it, and which
//! steps form the visual lane that runs beside the main walk.
//!
//! **Role:** the one table the runner, the resume check and the workers consult; the order is
//! `StepName::ALL`; which per-frame tables each step writes and reads; the documents each step
//! writes are named by `work_dir::store::keys`.
//!
//! **Position:** used by `runner`, `resume` and `workers`, and by the app's job queue for the
//! visual lane.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** a step reads only steps before it; the visual lane's steps read only the
//! probe, the shot scan and earlier lane steps, and the lane's start point comes after the shot
//! scan and before every lane step, so the lane runs beside the main walk from there; every GPU
//! step runs in a worker; the Whisper steps run in the ggml binary and nothing else does; a
//! step's revision changes whenever its code changes what it writes, so older outputs are not
//! reused.

use std::time::Duration;

use job_model::StepName;
use job_model::job::JobSettings;
use serde_json::{Value, json};
use worker_channel::address::{Address, Table};

use crate::work_dir::store::keys;

mod gpu;

pub use gpu::{
    GpuPriority, TEXT_DETECT_VRAM_MIB, VRAM_HEADROOM_MIB, WORKER_VRAM_CAP_MIB, gpu_priority,
    locks_gpu_lazily, vram_need_mib,
};

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

/// Whether the step's fingerprint covers the owner's line corrections.
pub fn reads_corrections(step: StepName) -> bool {
    step == StepName::Review
}

/// Whether the step reads the owner's on-screen text corrections, and its fingerprint covers
/// them while on-screen text is on.
pub fn reads_text_corrections(step: StepName) -> bool {
    matches!(
        step,
        StepName::TextRead | StepName::TextTranslate | StepName::TextReview
    )
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
        TextRead => &[ShotScan, TextDetect],
        TextTrack => &[ProbeDecode, ShotScan, TextRead],
        TextTranslate => &[ShotScan, TextTrack, Cues],
        TextReview => &[ProbeDecode, ShotScan, TextTranslate],
        TextMask => &[ProbeDecode, TextReview],
        TextInpaint => &[TextMask],
        TextCompose => &[TextReview, TextInpaint],
        TextVerify => &[ProbeDecode, TextReview, TextCompose],
        TextTypeset => &[TextReview],
        Qc => &[
            ProbeDecode,
            Vad,
            AsrParakeet,
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

/// The visual lane: the steps the runner walks on a thread of their own beside the main walk,
/// since they read only the probe, the shot scan and each other.
pub const VISUAL_LANE: [StepName; 3] = [
    StepName::TextDetect,
    StepName::TextRead,
    StepName::TextTrack,
];

/// Where the visual lane starts: the first step that waits on Claude, whose wait leaves the GPU and
/// the CPU idle. The lane's steps read only the probe, the shot scan and each other, all finished
/// by then.
pub const VISUAL_LANE_STARTS_AT: StepName = StepName::Adjudicate;

/// Where the main walk waits for the visual lane: the first step that reads a lane step.
pub const VISUAL_LANE_JOINS_AT: StepName = StepName::TextTranslate;

/// Whether `step` runs in the visual lane rather than the main walk.
pub fn in_visual_lane(step: StepName) -> bool {
    VISUAL_LANE.contains(&step)
}

/// Whether `step` reads a step of the visual lane, so the main walk waits for the lane before it.
pub fn reads_visual_lane(step: StepName) -> bool {
    inputs(step).iter().any(|input| in_visual_lane(*input))
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
    // Full-resolution frames screened in padded batches on two sessions; small, flat and
    // short-lived regions are dropped before they are tracked, writing is bisected once it
    // persists, also across screening groups, and only persistent, confident keyframes are
    // confirmed.
    (StepName::TextDetect, 6),
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
    // sign; every frame's mask is a `frames` row, a plate's erase the union of its frames', and
    // a plate whose frames take several shifts has one patch per shift.
    // Region crops are decoded as YUV and converted in the source's own matrix and range.
    (StepName::TextMask, 4),
    (StepName::TextCompose, 3),
    // Strokes left after the wider retry no longer decide; the read-back check does.
    (StepName::TextInpaint, 4),
    // A local OCR reads each finished replacement back before it reaches the localized video;
    // its readings are `readings` rows, and each frame blends the patch of its own shift.
    (StepName::TextVerify, 3),
    // Only the keyframe-bounded segments with replaced writing are re-encoded, as H.264 matching
    // the source; the rest of the source's bitstream is copied.
    (StepName::LocalizedVideo, 4),
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
        // The detectors' engine changes the boxes they find; how frames are decoded does not.
        TextDetect => json!({
            "enabled": settings.onscreen_text.enabled,
            "detector_engine": settings.onscreen_text.detector_engine,
        }),
        TextRead | TextTrack | TextReview => {
            json!({ "enabled": settings.onscreen_text.enabled })
        }
        TextMask | TextInpaint | TextCompose | TextVerify | TextTypeset => json!({
            "enabled": settings.onscreen_text.enabled,
            "localized_video": settings.onscreen_text.localized_video,
        }),
        LocalizedVideo => json!({
            "enabled": settings.onscreen_text.enabled,
            "localized_video": settings.onscreen_text.localized_video,
            "localized_encoder": settings.onscreen_text.localized_encoder,
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

/// Every stored value `step` reads: the job record, every document of every step it reads, the
/// owner's corrections it depends on, and, for the steps that replace a file of their own earlier
/// run beside the video, that run's record. A worker receives them down its stdin.
pub fn reads(step: StepName) -> Vec<Address> {
    let mut reads = vec![keys::job_record_address()];
    for input in inputs(step) {
        reads.extend(
            keys::output_parts(*input)
                .iter()
                .map(|part| keys::output_address(*input, *part)),
        );
    }
    if reads_corrections(step) || step == StepName::Qc {
        reads.push(keys::corrections_address(keys::LINE_CORRECTIONS));
    }
    if reads_text_corrections(step) {
        reads.push(keys::corrections_address(keys::TEXT_CORRECTIONS));
    }
    if reads_its_earlier_run(step) {
        reads.push(keys::output_address(step, None));
    }
    reads
}

/// The per-frame tables `step` writes rows of: the stroke masks write `frames`, the read-back
/// check `readings`. The step owns the table: its rows go whenever the step runs again.
pub fn writes_rows(step: StepName) -> &'static [Table] {
    match step {
        StepName::TextMask => &[Table::Frames],
        StepName::TextVerify => &[Table::Readings],
        _ => &[],
    }
}

/// The per-frame tables whose rows `step` reads, all of them, in key order: composition, the
/// read-back check and the localized video place each frame's lettering by its `frames` row. A
/// worker receives them after the documents [`reads`] names, one `Input` frame per row, and reads
/// them one at a time.
pub fn reads_rows(step: StepName) -> &'static [Table] {
    match step {
        StepName::TextCompose | StepName::TextVerify | StepName::LocalizedVideo => &[Table::Frames],
        _ => &[],
    }
}

/// Whether the step reads the record of its own earlier run: the output and the localized video
/// replace the files they wrote beside the video before.
pub fn reads_its_earlier_run(step: StepName) -> bool {
    matches!(step, StepName::Output | StepName::LocalizedVideo)
}

/// Whether a value `step` [`reads`] may be missing: the owner's corrections, of which a job has
/// none until the owner makes one, and the record of the step's own earlier run.
pub fn is_optional_read(step: StepName, address: &Address) -> bool {
    address.table == Table::Corrections
        || (reads_its_earlier_run(step) && *address == keys::output_address(step, None))
}

/// Every step that reads `step`, directly or through other steps, in `StepName::ALL` order.
pub fn dependents(step: StepName) -> Vec<StepName> {
    let mut found: Vec<StepName> = Vec::new();
    for later in StepName::ALL {
        if inputs(later)
            .iter()
            .any(|input| *input == step || found.contains(input))
        {
            found.push(later);
        }
    }
    found
}

#[cfg(test)]
#[path = "tests/graph.rs"]
mod tests;
