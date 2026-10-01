//! CrispASR sessions: Whisper transcription and the Qwen3 forced aligner, on ggml with CUDA.
//!
//! **Role:** open a Whisper `ggml-*.bin` model once and transcribe 16 kHz mono chunks into words
//! with times and probabilities; run the Qwen3-ForcedAligner over a transcript and its audio.
//!
//! **Position:** used by the speech recognition stage and its Whisper step in the ggml worker,
//! and by the ggml stack spike tool (the only aligner caller), when the `crispasr` feature is on. crispasr builds `libcrispasr` and ggml from the pinned git
//! tag with cmake and nvcc; a binary finds them through the rpath its own build script sets.
//!
//! **Signals and state:** one CrispASR session per model; the aligner loads its model per call.
//!
//! **Invariants:** word times are relative to the audio handed in; Whisper runs with the
//! language fixed to English, so it never detects or translates.

use job_model::outputs::TimedWord;

/// An opened Whisper model.
pub struct Whisper {
    session: crispasr::Session,
    name: String,
}

impl Whisper {
    /// Open `model` (a whisper.cpp `ggml-*.bin`) with `threads` CPU threads for the parts ggml
    /// keeps on the CPU.
    pub fn open(model: &std::path::Path, name: &str, threads: i32) -> Result<Whisper, String> {
        let path = model.to_string_lossy();
        let session = crispasr::Session::open_with_backend(&path, "whisper", threads)?;
        Ok(Whisper {
            session,
            name: name.to_string(),
        })
    }

    /// The model name, as recorded in transcripts.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The words of one 16 kHz mono chunk, timed from its start.
    pub fn transcribe(&mut self, samples: &[f32]) -> Result<Vec<TimedWord>, String> {
        let segments = self.session.transcribe_with_language(samples, Some("en"))?;
        Ok(segments
            .into_iter()
            .flat_map(|segment| segment.words)
            .filter(|w| !w.text.trim().is_empty())
            .map(|w| TimedWord {
                text: w.text.trim().to_string(),
                start_s: w.start,
                end_s: w.end,
                confidence: Some(w.confidence),
            })
            .collect())
    }
}

/// A word the aligner timed.
#[derive(Debug, Clone, PartialEq)]
pub struct AlignedTime {
    pub text: String,
    pub start_s: f64,
    pub end_s: f64,
}

/// Time `transcript` against `samples` (16 kHz mono) with the Qwen3 forced aligner at `model`;
/// times are relative to the samples' start.
pub fn align_qwen3(
    model: &std::path::Path,
    transcript: &str,
    samples: &[f32],
    threads: i32,
) -> Result<Vec<AlignedTime>, String> {
    let words = crispasr::align_words(&model.to_string_lossy(), transcript, samples, 0.0, threads)?;
    Ok(words
        .into_iter()
        .map(|w| AlignedTime {
            text: w.text,
            start_s: w.start,
            end_s: w.end,
        })
        .collect())
}
