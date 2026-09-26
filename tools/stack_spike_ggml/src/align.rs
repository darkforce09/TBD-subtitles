//! The Qwen3 aligner item: Parakeet's transcript re-timed by Qwen3-ForcedAligner on the vocal stem.
//!
//! **Role:** for every chunk, hand the Parakeet-TDT words and the RoFormer vocal stem to the Qwen3
//! forced aligner, match its words back to the displayed words, and compare the times with the
//! TDT times.
//!
//! **Position:** called by `items.rs`; uses `inference::ggml::crispasr::align_qwen3`,
//! `stages::alignment::checks` and `stages::diff_sheet::align` for the word matching.
//!
//! **Signals and state:** reads `asr.parakeet.mix.json` and `vocals_16k.roformer.f32`; writes
//! `aligned.qwen3.json`.
//!
//! **Invariants:** the words are the TDT transcript's; an aligner word that matches no displayed
//! word is dropped, and a displayed word it did not time stays untimed.

#![cfg(feature = "crispasr")]

use std::path::Path;
use std::time::Instant;

use job_model::outputs::{ChunkWords, EngineTranscript, ProbeResult, TimedWord};
use media_io::pcm_stream::read_f32_range;
use stages::alignment::checks;
use stages::diff_sheet::align::{self, Step};

use crate::report::Outcome;

const MODEL: (&str, &str) = (
    "qwen3-forced-aligner-0.6b",
    "qwen3-forced-aligner-0.6b-q8_0.gguf",
);

pub(crate) fn run(work: &Path) -> anyhow::Result<Outcome> {
    let transcript: EngineTranscript = serde_json::from_str(&std::fs::read_to_string(
        work.join("asr.parakeet.mix.json"),
    )?)?;
    let probe: ProbeResult =
        serde_json::from_str(&std::fs::read_to_string(work.join("probe.json"))?)?;
    let model = inference::model_store::models_dir()?
        .join(MODEL.0)
        .join(MODEL.1);
    let mut outcome = Outcome {
        audio_s: probe.duration_s,
        ..Outcome::default()
    };
    let started = Instant::now();
    let (mut aligned_all, mut reference_all, mut chunks) = (Vec::new(), Vec::new(), Vec::new());
    let mut empty_chunks = 0;
    for chunk in &transcript.chunks {
        if chunk.words.is_empty() {
            continue;
        }
        let start = (chunk.span.start_s * 16_000.0) as u64;
        let count = (chunk.span.duration_s() * 16_000.0) as usize;
        let samples = read_f32_range(&work.join("vocals_16k.roformer.f32"), start, count)?;
        let text = chunk
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let timed = inference::ggml::crispasr::align_qwen3(&model, &text, &samples, 8)
            .map_err(anyhow::Error::msg)?;
        if timed.is_empty() {
            empty_chunks += 1;
        }
        let display = align::normalise_all(chunk.words.iter().map(|w| w.text.as_str()));
        let got = align::normalise_all(timed.iter().map(|w| w.text.as_str()));
        let mut times = vec![None; chunk.words.len()];
        // Displayed words that fold to nothing were dropped from `display`; map indices back.
        let kept: Vec<usize> = (0..chunk.words.len())
            .filter(|&i| !align::normalise(&chunk.words[i].text).is_empty())
            .collect();
        let got_index: Vec<usize> = (0..timed.len())
            .filter(|&i| !align::normalise(&timed[i].text).is_empty())
            .collect();
        for step in align::align(&display, &got) {
            if let Step::Match(i, j) = step {
                let w = &timed[got_index[j]];
                times[kept[i]] =
                    Some((w.start_s + chunk.span.start_s, w.end_s + chunk.span.start_s));
            }
        }
        let mut words = Vec::new();
        for (word, time) in chunk.words.iter().zip(&times) {
            aligned_all.push(*time);
            reference_all.push((word.start_s, word.end_s));
            if let Some((a, b)) = time {
                words.push(TimedWord {
                    text: word.text.clone(),
                    start_s: *a,
                    end_s: *b,
                    confidence: None,
                });
            }
        }
        chunks.push(ChunkWords {
            span: chunk.span,
            words,
        });
    }
    outcome.process_s = started.elapsed().as_secs_f64();
    let comparison = checks::compare(&aligned_all, &reference_all);
    let timed: Vec<(f64, f64)> = aligned_all.iter().flatten().copied().collect();
    let out = EngineTranscript {
        engine: "qwen3-forced-aligner-0.6b".into(),
        input: "roformer".into(),
        chunks,
    };
    std::fs::write(
        work.join("aligned.qwen3.json"),
        serde_json::to_vec_pretty(&out)?,
    )?;
    outcome.note("empty_chunks", empty_chunks);
    outcome.note("words", reference_all.len());
    outcome.note("words_timed", comparison.words);
    outcome.note("median_start_diff_vs_tdt_s", comparison.median_start_diff_s);
    outcome.note("share_over_200ms_vs_tdt", comparison.share_over_200ms);
    outcome.note("suspicious_runs", checks::suspicious_runs(&timed));
    outcome.note("zero_length_runs", checks::flat_runs(&timed));
    Ok(outcome)
}
