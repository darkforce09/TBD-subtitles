//! The alignment of a whole job: blocks first, each failed block utterance by utterance, and the
//! backbone's own times when the aligner cannot be trusted; every word records its source.
//!
//! **Role:** drive a [`WordAligner`] over the planned blocks, check each result, fall back in
//! order (block, utterance alone, backbone, interpolation; for a corrected line, its earlier
//! aligned times before the backbone), and measure the job's offset from the backbone.
//!
//! **Position:** called by the alignment step inside the ONNX worker, whose aligner reads the
//! vocal stem and runs Parakeet-CTC; tests use a scripted aligner.
//!
//! **Signals and state:** one aligner call per block, plus one per utterance of a failed block.
//!
//! **Invariants:** every kept utterance comes out with one time per displayed word; words keep
//! their order; the offset is measured only on words the aligner timed in a passing block.

use job_model::outputs::{Aligned, AlignedUtterance, AlignedWord, TimeSpan, TimingSource};

use super::WordTimes;
use super::blocks::{Kept, audio_span, plan_blocks};
use super::timing::{backbone_times, carried_times, interpolate, passes, signed_median};

/// Something that times words against audio.
pub trait WordAligner {
    /// Times of `words` in video seconds within `span`; `Ok(None)` when the span is too short for
    /// the text.
    fn align(&mut self, span: TimeSpan, words: &[String]) -> Result<Option<WordTimes>, String>;
}

/// Align every kept utterance; `progress` hears `(blocks done, blocks)`.
pub fn align_all(
    kept: &[Kept],
    duration_s: f64,
    aligner: &mut dyn WordAligner,
    mut progress: impl FnMut(usize, usize),
) -> Aligned {
    let blocks = plan_blocks(kept);
    let mut result = Aligned {
        blocks: blocks.len(),
        ..Aligned::default()
    };
    let mut diffs = Vec::new();
    for (b, range) in blocks.iter().enumerate() {
        let group = &kept[range.clone()];
        let references: Vec<WordTimes> = group
            .iter()
            .map(|k| backbone_times(&k.words, &k.backbone))
            .collect();
        let words: Vec<String> = group.iter().flat_map(|k| k.words.iter().cloned()).collect();
        let span = audio_span(kept, range.clone(), duration_s);
        let split = attempt(
            aligner,
            span,
            &words,
            &mut result.errors,
            &format!("block {}", b + 1),
        )
        .map(|times| split_by(&times, group));
        let passed = split.as_ref().is_some_and(|per| {
            let parts: Vec<_> = group
                .iter()
                .zip(per)
                .zip(&references)
                .map(|((k, t), r)| (t.as_slice(), (k.start_s, k.end_s), r.as_slice()))
                .collect();
            passes(&parts)
        });
        match split {
            Some(per) if passed => {
                for ((k, times), reference) in group.iter().zip(&per).zip(&references) {
                    for (t, r) in times.iter().zip(reference) {
                        if let (Some(t), Some(r)) = (t, r) {
                            diffs.push(t.0 - r.0);
                        }
                    }
                    result.utterances.push(finish(k, times, TimingSource::Ctc));
                }
            }
            _ => {
                result.failed_blocks += 1;
                for (offset, (k, reference)) in group.iter().zip(&references).enumerate() {
                    let i = range.start + offset;
                    let span = audio_span(kept, i..i + 1, duration_s);
                    let alone = attempt(aligner, span, &k.words, &mut result.errors, &k.id);
                    let utterance = match alone {
                        Some(times) if passes(&[(&times, (k.start_s, k.end_s), reference)]) => {
                            finish(k, &times, TimingSource::CtcUtterance)
                        }
                        _ => finish(k, reference, TimingSource::Backbone),
                    };
                    result.utterances.push(utterance);
                }
            }
        }
        progress(b + 1, blocks.len());
    }
    result.offset_s = signed_median(&mut diffs);
    result
}

/// Time kept utterance `index` alone, as corrected: the aligner over the utterance's own span
/// (between the middles of the gaps to its neighbours), else the aligner's times it had `before`
/// carried over to the words that stayed or took another's place, else the backbone's times matched
/// to the new words, else interpolation. The result is never unsure.
pub fn realign_utterance(
    kept: &[Kept],
    index: usize,
    duration_s: f64,
    aligner: &mut dyn WordAligner,
    errors: &mut Vec<String>,
    before: Option<&AlignedUtterance>,
) -> AlignedUtterance {
    let k = &kept[index];
    let reference = backbone_times(&k.words, &k.backbone);
    let span = audio_span(kept, index..index + 1, duration_s);
    let mut utterance = match attempt(aligner, span, &k.words, errors, &k.id) {
        Some(times) if passes(&[(&times, (k.start_s, k.end_s), &reference)]) => {
            finish(k, &times, TimingSource::CtcUtterance)
        }
        _ => match before.and_then(|b| carried_times(&k.words, &b.words)) {
            Some(carried) => finish_each(k, &carried.times, &carried.sources),
            None => finish(k, &reference, TimingSource::Backbone),
        },
    };
    utterance.unsure = false;
    utterance
}

/// One aligner call; an error is recorded and read as a failed alignment.
fn attempt(
    aligner: &mut dyn WordAligner,
    span: TimeSpan,
    words: &[String],
    errors: &mut Vec<String>,
    what: &str,
) -> Option<WordTimes> {
    match aligner.align(span, words) {
        Ok(Some(times)) if times.len() == words.len() => Some(times),
        Ok(Some(times)) => {
            errors.push(format!(
                "{what}: {} times for {} words",
                times.len(),
                words.len()
            ));
            None
        }
        Ok(None) => None,
        Err(e) => {
            errors.push(format!("{what}: {e}"));
            None
        }
    }
}

/// A block's times cut back into its utterances.
fn split_by(times: &[Option<(f64, f64)>], group: &[Kept]) -> Vec<WordTimes> {
    let mut at = 0;
    group
        .iter()
        .map(|k| {
            let part = times[at..at + k.words.len()].to_vec();
            at += k.words.len();
            part
        })
        .collect()
}

/// The utterance with `times` from `source`, and every untimed word interpolated.
fn finish(k: &Kept, times: &[Option<(f64, f64)>], source: TimingSource) -> AlignedUtterance {
    finish_each(k, times, &vec![Some(source); times.len()])
}

/// The utterance with each word's time from its own source, and every untimed word interpolated.
fn finish_each(
    k: &Kept,
    times: &[Option<(f64, f64)>],
    sources: &[Option<TimingSource>],
) -> AlignedUtterance {
    let filled = interpolate(&k.words, times, k.start_s, k.end_s);
    let words = k
        .words
        .iter()
        .zip(times.iter().zip(sources))
        .zip(filled)
        .map(|((text, (time, source)), (start_s, end_s))| AlignedWord {
            text: text.clone(),
            start_s,
            end_s,
            source: match (time, source) {
                (Some(_), Some(source)) => *source,
                _ => TimingSource::Interpolated,
            },
        })
        .collect();
    AlignedUtterance {
        id: k.id.clone(),
        words,
        speaker_starts: k.speaker_starts.clone(),
        new_speaker: k.new_speaker,
        narrator: k.narrator,
        unsure: k.unsure,
    }
}

#[cfg(test)]
#[path = "tests/run.rs"]
mod tests;
