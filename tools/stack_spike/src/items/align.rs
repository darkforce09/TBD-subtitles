//! The CTC alignment item: Parakeet's transcript re-timed through Parakeet-CTC on the vocal stem.
//!
//! **Role:** for every chunk, run Parakeet-CTC over the RoFormer vocal stem, force-align the
//! Parakeet-TDT words of that chunk with CTC Viterbi, and compare the times with the TDT times.
//!
//! **Position:** called by `items/mod.rs` inside a GPU worker; uses
//! `inference::onnx::parakeet_ctc` and `stages::alignment`.
//!
//! **Signals and state:** reads `asr.parakeet.mix.json` and `vocals_16k.roformer.f32`; writes
//! `aligned.ctc.json`.
//!
//! **Invariants:** the words are the TDT transcript's, unchanged; only their times are new.

use std::time::Instant;

use inference::onnx::Device;
use inference::onnx::parakeet_ctc::{self, ParakeetCtc};
use job_model::outputs::{ChunkWords, EngineTranscript, TimedWord};
use media_io::pcm_stream::read_f32_range;
use stages::alignment::{self, checks};

use super::{Outcome, since};
use crate::context::Context;

/// The transcript every aligner re-times.
pub(crate) const TRANSCRIPT: &str = "asr.parakeet.mix.json";
/// The audio every aligner hears.
pub(crate) const AUDIO: &str = "vocals_16k.roformer.f32";

pub(crate) fn run(ctx: &Context) -> anyhow::Result<Outcome> {
    let transcript: EngineTranscript = ctx.read_json(TRANSCRIPT)?;
    let load = Instant::now();
    let mut model = ParakeetCtc::open(&ctx.models.join(parakeet_ctc::MODEL), Device::Cuda)?;
    let mut outcome = Outcome {
        audio_s: ctx.probe()?.duration_s,
        load_s: since(load),
        ..Outcome::default()
    };
    let started = Instant::now();
    let mut model_s = 0.0;
    let mut chunks = Vec::new();
    let (mut aligned_all, mut reference_all) = (Vec::new(), Vec::new());
    let mut failed_chunks = 0;
    for chunk in &transcript.chunks {
        let start = (chunk.span.start_s * 16_000.0) as u64;
        let count = (chunk.span.duration_s() * 16_000.0) as usize;
        let samples = read_f32_range(&ctx.path(AUDIO), start, count)?;
        let run = Instant::now();
        let grid = model.log_probs(&samples)?;
        model_s += since(run);
        if chunks.is_empty() && failed_chunks == 0 {
            let nans = grid.log_probs.iter().filter(|x| x.is_nan()).count();
            outcome.note("first_chunk_nan_values", nans);
            outcome.note("first_chunk_frames", grid.frames);
            outcome.note("first_chunk_greedy_text", model.greedy_text(&grid)?);
        }
        let words: Vec<String> = chunk.words.iter().map(|w| w.text.clone()).collect();
        let times = alignment::align_words_ctc(
            &grid.log_probs,
            grid.vocab,
            parakeet_ctc::BLANK,
            parakeet_ctc::FRAME_S,
            &words,
            |w| model.tokens(w).map_err(|e| e.to_string()),
        )
        .map_err(anyhow::Error::msg)?;
        let Some(times) = times else {
            failed_chunks += 1;
            continue;
        };
        let mut timed = Vec::new();
        for (word, time) in chunk.words.iter().zip(&times) {
            let absolute = time.map(|(a, b)| (a + chunk.span.start_s, b + chunk.span.start_s));
            aligned_all.push(absolute);
            reference_all.push((word.start_s, word.end_s));
            if let Some((a, b)) = absolute {
                timed.push(TimedWord {
                    text: word.text.clone(),
                    start_s: a,
                    end_s: b,
                    confidence: None,
                });
            }
        }
        chunks.push(ChunkWords {
            span: chunk.span,
            words: timed,
        });
    }
    outcome.process_s = since(started);
    let comparison = checks::compare(&aligned_all, &reference_all);
    let timed: Vec<(f64, f64)> = aligned_all.iter().flatten().copied().collect();
    ctx.write_json(
        "aligned.ctc.json",
        &EngineTranscript {
            engine: "ctc-viterbi/parakeet-ctc-0.6b".into(),
            input: "roformer".into(),
            chunks,
        },
    )?;
    outcome.note("model_s", model_s);
    outcome.note("failed_chunks", failed_chunks);
    outcome.note("words_timed", comparison.words);
    outcome.note("median_start_diff_vs_tdt_s", comparison.median_start_diff_s);
    outcome.note("share_over_200ms_vs_tdt", comparison.share_over_200ms);
    outcome.note("suspicious_runs", checks::suspicious_runs(&timed));
    Ok(outcome)
}
