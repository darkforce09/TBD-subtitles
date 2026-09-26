//! Forced alignment of the final text against the vocal stem, with checks that catch a failed
//! alignment.
//!
//! **Role:** time every displayed word of a block from a CTC grid (`ctc_viterbi.rs` over the
//! spoken form of the words), and measure an alignment against reference times (`checks.rs`).
//!
//! **Position:** called by the alignment stage and the stack spike tool; the CTC grid and the
//! tokenizer come from `inference::onnx::parakeet_ctc`, passed in so this module stays pure.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** a displayed word is timed from its first spoken token's first frame to its
//! last token's last frame; a word with nothing speakable gets no time.

pub mod blocks;
pub mod checks;
pub mod ctc_viterbi;
pub mod run;
pub mod spoken_form;
pub mod timing;

/// `(start, end)` in block seconds per displayed word; `None` for a word with nothing to say.
pub type WordTimes = Vec<Option<(f64, f64)>>;

/// Time `words` (displayed) through a CTC grid. `tokens` spells one spoken word in the model's
/// tokens. Returns `None` when the grid is too short for the text; otherwise one `(start, end)`
/// in block seconds per displayed word, or `None` for a word with nothing to say.
pub fn align_words_ctc(
    log_probs: &[f32],
    vocab: usize,
    blank: usize,
    frame_s: f64,
    words: &[String],
    mut tokens: impl FnMut(&str) -> Result<Vec<usize>, String>,
) -> Result<Option<WordTimes>, String> {
    let mut sequence = Vec::new();
    let mut ranges = Vec::with_capacity(words.len());
    for word in words {
        let start = sequence.len();
        for spoken in spoken_form::spoken(word) {
            sequence.extend(tokens(&spoken)?);
        }
        ranges.push(start..sequence.len());
    }
    let Some(spans) = ctc_viterbi::align(log_probs, vocab, &sequence, blank) else {
        return Ok(None);
    };
    Ok(Some(
        ranges
            .into_iter()
            .map(|range| {
                if range.is_empty() {
                    return None;
                }
                let first = spans[range.start].0;
                let last = spans[range.end - 1].1;
                Some((first as f64 * frame_s, last as f64 * frame_s))
            })
            .collect(),
    ))
}
