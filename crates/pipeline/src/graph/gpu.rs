//! How each GPU step takes the card: the device memory it needs free before its worker starts,
//! whether it takes the GPU lock up front or only when its model loads, and its place in the
//! queue when the main walk and the visual lane both wait for the lock.
//!
//! **Role:** the GPU columns of the step table, read by `workers::run_worker` and the memory
//! wait.
//!
//! **Position:** a part of `graph`; its values come from the measured peaks of the baseline runs
//! (`documentation/research/m6_baseline.md` and the separation-limit snapshot).
//!
//! **Signals and state:** none; constants.
//!
//! **Invariants:** every step that uses the GPU has a memory need, none above the worker cap; a
//! need is a measured peak plus headroom, never the cap itself, so a step that fits beside the
//! desktop never waits; only the visual lane's steps queue behind the audio steps.

use job_model::StepName;

use super::{in_visual_lane, uses_gpu};

/// The VRAM one GPU worker may hold, in MiB: the card's 8 GiB less the desktop's share.
pub const WORKER_VRAM_CAP_MIB: u64 = 6_656;

/// Headroom over a measured peak, in MiB, for allocator growth between runs.
pub const VRAM_HEADROOM_MIB: u64 = 256;

/// The `text_detect` worker's VRAM need, in MiB: the larger phase's measured peak on Dressrosa
/// 11 and 28 plus `VRAM_HEADROOM_MIB`. Screening on CUDA, two threads each with a full-resolution
/// and a proxy session, peaked at 4,087 MiB; confirming at 3,347; TensorRT held less.
pub const TEXT_DETECT_VRAM_MIB: u64 = 4_087 + VRAM_HEADROOM_MIB;

/// The largest VRAM each GPU step was measured to hold, in MiB, on Dressrosa 11 and 28.
const MEASURED_PEAKS_MIB: &[(StepName, u64)] = &[
    (StepName::Separation, 4_174),
    (StepName::AsrParakeet, 3_404),
    (StepName::AsrWhisper, 4_412),
    (StepName::SoundEvents, 1_202),
    (StepName::RedecodeParakeet, 3_404),
    (StepName::RedecodeWhisper, 4_412),
    (StepName::Alignment, 3_458),
    (StepName::TextRead, 1_088),
    (StepName::TextTranslate, 3_566),
    (StepName::TextInpaint, 1_202),
    (StepName::TextVerify, 2_894),
    (StepName::LocalizedVideo, 275),
];

/// The device memory, in MiB, that must be free before `step`'s worker loads its model; `None`
/// for a step that does not use the GPU.
pub fn vram_need_mib(step: StepName) -> Option<u64> {
    if !uses_gpu(step) {
        return None;
    }
    if step == StepName::TextDetect {
        return Some(TEXT_DETECT_VRAM_MIB);
    }
    let peak = MEASURED_PEAKS_MIB
        .iter()
        .find(|(measured, _)| *measured == step)
        .map_or(WORKER_VRAM_CAP_MIB - VRAM_HEADROOM_MIB, |(_, peak)| *peak);
    Some((peak + VRAM_HEADROOM_MIB).min(WORKER_VRAM_CAP_MIB))
}

/// Whether `step`'s worker takes the GPU lock only when it loads its model rather than before it
/// starts: the translation step asks Claude first and loads the local model only for what Claude
/// leaves, so it holds the card only while that model is loaded.
pub fn locks_gpu_lazily(step: StepName) -> bool {
    step == StepName::TextTranslate
}

/// Which waiter takes the GPU lock first when several steps of one process wait for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuPriority {
    /// The main walk's steps: the audio chain whose wait delays the whole job.
    Audio,
    /// The visual lane's steps, which run beside the main walk and give way to it.
    Visual,
}

/// The priority `step` waits for the GPU lock with.
pub fn gpu_priority(step: StepName) -> GpuPriority {
    if in_visual_lane(step) {
        GpuPriority::Visual
    } else {
        GpuPriority::Audio
    }
}

#[cfg(test)]
#[path = "tests/gpu.rs"]
mod tests;
