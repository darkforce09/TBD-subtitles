//! What `job.json` records: the video it is for, the settings, and each finished step with its
//! fingerprint, finish time and measurements; plus what a worker process reports about itself.
//!
//! **Role:** the record the job runner keeps of one job and resumes from.
//!
//! **Position:** written and read by `pipeline`; built from the settings the app gives it.
//!
//! **Signals and state:** none; plain data, written as JSON and archived with rkyv.
//!
//! **Invariants:** a step is in `steps` only once it finished; a measure of `None` means not
//! measured, never zero.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::settings::JobSettings;
use crate::stage::StepName;

/// The record of one job.
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
pub struct JobRecord {
    /// The video, as given.
    pub video: String,
    pub video_size: u64,
    /// The video's modification time, in seconds since the Unix epoch.
    pub video_modified_s: i64,
    pub settings: JobSettings,
    /// The folder the models are read from, for this run; `None` means the default one. It never
    /// changes a step's output, so no fingerprint covers it.
    #[serde(default)]
    pub models_dir: Option<String>,
    /// The SHA-256 of the owner's corrections (`review.json`) when this run started; `None` when
    /// there are none. The review step's fingerprint covers it.
    #[serde(default)]
    pub corrections: Option<String>,
    /// Every step that finished, with what it was run on.
    #[serde(default)]
    pub steps: BTreeMap<StepName, StepRecord>,
}

/// One finished step.
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
pub struct StepRecord {
    /// The hash of the step's settings and of what its inputs were, when it ran.
    pub fingerprint: String,
    /// When it finished, in nanoseconds since the Unix epoch; a later step's fingerprint covers
    /// it, so re-running a step re-runs every step after it.
    pub finished_ns: u128,
    pub measure: StepMeasure,
}

/// A step's time and memory. `None` means not measured, never zero.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct StepMeasure {
    /// Wall time around the whole step, in seconds.
    pub wall_s: f64,
    /// Time to load models, in seconds.
    pub load_s: Option<f64>,
    /// Time to process the video once loaded, in seconds.
    pub process_s: Option<f64>,
    /// The step's own peak resident memory, in MiB.
    pub peak_ram_mib: Option<f64>,
    /// The largest child process's peak resident memory (FFmpeg, `claude`), in MiB.
    pub peak_child_ram_mib: Option<f64>,
    /// The step's peak GPU memory, in MiB.
    pub peak_vram_mib: Option<f64>,
    /// Short facts about the run, such as a count of chunks or calls.
    #[serde(default)]
    pub notes: BTreeMap<String, String>,
}

/// What a worker process writes about itself when its step finishes.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct WorkerMeasure {
    pub load_s: f64,
    pub process_s: f64,
    pub peak_ram_mib: f64,
    pub peak_child_ram_mib: f64,
    #[serde(default)]
    pub notes: BTreeMap<String, String>,
}

#[cfg(test)]
#[path = "tests/record.rs"]
mod tests;
