//! The pipeline stages.
//!
//! **Role:** one module folder per stage, from probing the video to writing the subtitle file.
//! Each stage reads its inputs from the job's work directory and writes one typed output there.
//!
//! **Position:** called by `pipeline`, in process for CPU stages and inside a `worker` process
//! for GPU stages; uses `media_io`, `inference`, `subtitle_formats` and `job_model`.
//!
//! **Signals and state:** reads and writes the job's work directory only; the output stage, and
//! `localize` at the path its task hands it, alone write beside the video.
//!
//! **Invariants:** a stage's output is complete or absent, never partial; no stage invents a word
//! that no speech engine heard.

pub mod adjudication;
pub mod alignment;
pub mod asr;
pub mod cues;
pub mod diff_sheet;
pub mod fix_it;
pub mod localize;
pub mod onscreen_text;
pub mod output;
pub mod probe_decode;
pub mod qc;
pub mod separation;
pub mod sound_events;
pub mod vad;
