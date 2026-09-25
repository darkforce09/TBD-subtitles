//! Each speech engine over the chunk plan: words with times and confidences.
//!
//! **Role:** feed every chunk of the plan, cut from a 16 kHz mono file, to one speech engine and
//! collect its words with times moved from the chunk to the video.
//!
//! **Position:** called inside the speech recognition worker (and by the stack spike tool);
//! `engines.rs` adapts the `inference` backends to [`SpeechEngine`].
//!
//! **Signals and state:** reads the audio file one chunk at a time; holds one chunk's samples.
//!
//! **Invariants:** every engine hears exactly the same chunks; every word time is in video
//! seconds and inside its chunk (clamped when an engine overshoots).

pub mod engines;

use std::path::Path;

use job_model::outputs::{ChunkWords, EngineTranscript, SpeechPlan, TimedWord};
use media_io::pcm_stream::read_f32_range;

/// Samples per second of the input files.
pub const RATE: f64 = 16_000.0;

/// A speech engine: 16 kHz mono samples in, words timed from the chunk's start out.
pub trait SpeechEngine {
    /// The engine and model, as recorded in the transcript.
    fn name(&self) -> String;
    fn transcribe(&mut self, samples: &[f32]) -> Result<Vec<TimedWord>, String>;
}

/// Run `engine` over every chunk of `plan`, reading `audio` (a 16 kHz mono `.f32` file).
pub fn transcribe_plan(
    engine: &mut dyn SpeechEngine,
    audio: &Path,
    input: &str,
    plan: &SpeechPlan,
    mut progress: impl FnMut(usize, usize),
) -> Result<EngineTranscript, String> {
    let mut chunks = Vec::with_capacity(plan.chunks.len());
    for (i, span) in plan.chunks.iter().enumerate() {
        let start = (span.start_s * RATE) as u64;
        let count = (span.duration_s() * RATE) as usize;
        let samples = read_f32_range(audio, start, count).map_err(|e| e.to_string())?;
        let words = engine.transcribe(&samples)?;
        chunks.push(ChunkWords {
            span: *span,
            words: shift(words, span.start_s, span.end_s),
        });
        progress(i + 1, plan.chunks.len());
    }
    Ok(EngineTranscript {
        engine: engine.name(),
        input: input.to_string(),
        chunks,
    })
}

/// Move chunk-relative times to video time, clamped inside the chunk.
pub fn shift(words: Vec<TimedWord>, start_s: f64, end_s: f64) -> Vec<TimedWord> {
    words
        .into_iter()
        .map(|w| {
            let s = (w.start_s + start_s).clamp(start_s, end_s);
            TimedWord {
                start_s: s,
                end_s: (w.end_s + start_s).clamp(s, end_s),
                ..w
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/asr.rs"]
mod tests;
