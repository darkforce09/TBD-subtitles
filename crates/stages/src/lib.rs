//! The pipeline stages.
//!
//! **Role:** one module folder per stage, from probing the video to writing the subtitle file
//! and the localized video. Each stage takes typed inputs and returns typed outputs; the job
//! runner reads the inputs from the job's store and stores the outputs.
//!
//! **Position:** called by `pipeline`, in process or inside a `worker` process as its step graph
//! places each step; uses `media_io`, `inference`, `subtitle_formats` and `job_model`.
//!
//! **Signals and state:** never opens the job's store; reads and writes media files (the mix,
//! stems, crops, masks, plates, patches) in the job's work directory only; the output stage, and
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
