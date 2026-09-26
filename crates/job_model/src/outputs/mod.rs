//! The typed output of each stage, one JSON file per step in the job's work directory.

pub mod adjudication;
pub mod aligned;
pub mod output;
pub mod probe;
pub mod sheet;
pub mod shots;
pub mod sound_cues;
pub mod sound_events;
pub mod speech;
pub mod words;

pub use adjudication::{AdjudicationPass, Findings, Line, Redecode};
pub use aligned::{Aligned, AlignedUtterance, AlignedWord, TimingSource};
pub use output::OutputRecord;
pub use probe::{AudioStream, ProbeDecoded, ProbeResult, VideoStream};
pub use sheet::Utterance;
pub use shots::{ShotChanges, ShotCut};
pub use sound_cues::{CandidateKind, SoundCandidate, SoundCue, SoundCues};
pub use sound_events::SoundEvent;
pub use speech::{SpeechPlan, TimeSpan};
pub use words::{ChunkWords, EngineTranscript, TimedWord};
