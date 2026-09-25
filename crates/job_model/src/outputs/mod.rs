//! The typed output of each stage, one JSON file per stage in the job's work directory.

pub mod probe;
pub mod shots;
pub mod speech;

pub use probe::{AudioStream, ProbeResult, VideoStream};
pub use shots::{ShotChanges, ShotCut};
pub use speech::{SpeechPlan, TimeSpan};
