//! The job runner.
//!
//! **Role:** run one video through every step of the pipeline: the step graph, resume from
//! finished steps, GPU steps as worker processes of the app's binaries, the time and peak memory
//! of every step, progress events, and the job report.
//!
//! **Position:** layer 3; called by the app (`process`, `fix`, `dump`, `worker` and the window)
//! and its worker binaries; calls `stages`, the backends in `inference`, `media_io`,
//! `subtitle_formats`, `child_process` and `worker_channel`.
//!
//! **Signals and state:** owns the job's database, `job.redb`, and writes the large media files of
//! the job's work directory; the output step writes the subtitle file beside the video; reads and
//! records the sign library, `library.redb`; starts worker processes.
//!
//! **Invariants:** one GPU worker runs at a time; a killed job leaves every finished step valid
//! for resume; ONNX Runtime, ggml and candle never load into one process.

pub mod cancel;
pub mod error;
pub mod fix_it;
pub mod graph;
pub mod library;
pub mod measure;
pub mod models;
pub mod progress;
pub mod report;
pub mod resume;
pub mod runner;
pub mod tasks;
pub mod work_dir;
pub mod workers;

pub use cancel::CancelToken;
pub use error::{PipelineError, Result};
pub use runner::{JobOptions, JobOutcome, run_job};
