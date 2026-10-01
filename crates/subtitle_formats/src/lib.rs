//! Subtitle cues and the files they are written to.
//!
//! **Role:** the cue model and one writer per format (SRT, WebVTT, ASS), plus import of existing
//! subtitle files.
//!
//! **Position:** called by `stages` (cue building, on-screen text, quality check), by `pipeline`
//! (the job store and the output step) and by the visual validation tool; declares `job_model`.
//!
//! **Signals and state:** writers return text; import reads the files it is given.
//!
//! **Invariants:** every file is UTF-8; a writer never changes a cue's times or text.

pub mod cue;
pub mod import;
pub mod writers;
