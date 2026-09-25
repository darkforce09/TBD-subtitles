//! The typed output of each stage, one JSON file per stage in the job's work directory.

pub mod probe;
pub mod shots;
pub mod sound_events;
pub mod speech;
pub mod words;

pub use probe::{AudioStream, ProbeResult, VideoStream};
pub use shots::{ShotChanges, ShotCut};
pub use sound_events::SoundEvent;
pub use speech::{SpeechPlan, TimeSpan};
pub use words::{ChunkWords, EngineTranscript, TimedWord};
