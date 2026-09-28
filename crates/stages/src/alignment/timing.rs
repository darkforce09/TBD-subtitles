//! Word times without the aligner, and the checks an aligned block or utterance must pass.
//!
//! **Role:** map the backbone engine's word times onto the displayed words; carry an utterance's
//! earlier aligned times over to its corrected words; spread words no source timed between their
//! timed neighbours; and judge an alignment against the recognition window and the backbone.
//!
//! **Position:** used by `run.rs`.
//!
//! **Signals and state:** none; pure.
//!
//! **Invariants:** interpolated times stay inside their neighbours and keep word order; a check
//! never passes an alignment it could not measure against at least one rule.

use job_model::outputs::{AlignedWord, TimedWord, TimingSource};

use super::checks;
use crate::diff_sheet::align::{self, Step};

/// An utterance's aligned speech must lie within this of its recognition window.
pub const WINDOW_TOLERANCE_S: f64 = 1.0;
/// The median start difference from the backbone above which an alignment fails.
pub const MAX_MEDIAN_DIFF_S: f64 = 0.2;

/// The backbone's time for each displayed word it lines up with (a match or a substitution).
pub fn backbone_times(display: &[String], backbone: &[TimedWord]) -> Vec<Option<(f64, f64)>> {
    // Unfiltered, so the indices stay those of the words.
    let a: Vec<String> = display.iter().map(|w| align::normalise(w)).collect();
    let b: Vec<String> = backbone.iter().map(|w| align::normalise(&w.text)).collect();
    let mut times = vec![None; display.len()];
    for step in align::align(&a, &b) {
        if let Step::Match(i, j) | Step::Substitute(i, j) = step {
            times[i] = Some((backbone[j].start_s, backbone[j].end_s));
        }
    }
    times
}

/// The aligner's earlier times of an utterance carried over to its corrected words, when the
/// aligner cannot time the corrected line alone: a word that stayed keeps its time and source; a
/// run of words that replaced a run of aligner-timed words shares out that run's span by
/// characters; a new word with nothing replaced gets none, to be spread between its neighbours.
/// `None` when the aligner timed no word of `before`.
pub fn carried_times(words: &[String], before: &[AlignedWord]) -> Option<Carried> {
    if !before.iter().any(by_aligner) {
        return None;
    }
    let old: Vec<String> = before.iter().map(|w| align::normalise(&w.text)).collect();
    let new: Vec<String> = words.iter().map(|w| align::normalise(w)).collect();
    let mut carried = Carried {
        times: vec![None; words.len()],
        sources: vec![None; words.len()],
    };
    let (mut i, mut j) = (0, 0);
    for (mi, mj) in common_words(&old, &new) {
        carried.share(words, &before[i..mi], j..mj);
        carried.share(words, &before[mi..=mi], mj..mj + 1);
        (i, j) = (mi + 1, mj + 1);
    }
    carried.share(words, &before[i..], j..new.len());
    Some(carried)
}

/// Whether the aligner timed `word`.
fn by_aligner(word: &AlignedWord) -> bool {
    matches!(word.source, TimingSource::Ctc | TimingSource::CtcUtterance)
}

/// Each corrected word's carried time and its source; `None` for a word that carries none.
pub struct Carried {
    pub times: Vec<Option<(f64, f64)>>,
    pub sources: Vec<Option<TimingSource>>,
}

impl Carried {
    /// Share the span of `replaced`, every word of it timed by the aligner, among the words of
    /// `words` in `run`, by characters; nothing when either side is empty.
    fn share(&mut self, words: &[String], replaced: &[AlignedWord], run: std::ops::Range<usize>) {
        if run.is_empty() || replaced.is_empty() || !replaced.iter().all(by_aligner) {
            return;
        }
        let (from, to) = (replaced[0].start_s, replaced[replaced.len() - 1].end_s);
        let weights: Vec<f64> = words[run.clone()]
            .iter()
            .map(|w| w.chars().count().max(1) as f64)
            .collect();
        let total: f64 = weights.iter().sum();
        let mut at = from;
        for (j, weight) in run.zip(weights) {
            let next = at + (to - from) * weight / total;
            self.times[j] = Some((at, next));
            self.sources[j] = Some(replaced[0].source);
            at = next;
        }
    }
}

/// The pairs `(i, j)` of a longest run of words `a[i] == b[j]` both lists share, in order.
fn common_words(a: &[String], b: &[String]) -> Vec<(usize, usize)> {
    let (n, m) = (a.len(), b.len());
    let mut longest = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            longest[i][j] = if a[i] == b[j] {
                longest[i + 1][j + 1] + 1
            } else {
                longest[i + 1][j].max(longest[i][j + 1])
            };
        }
    }
    let (mut i, mut j, mut pairs) = (0, 0, Vec::new());
    while i < n && j < m {
        if a[i] == b[j] {
            pairs.push((i, j));
            (i, j) = (i + 1, j + 1);
        } else if longest[i + 1][j] >= longest[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pairs
}

/// Fill each `None` by spreading the untimed run evenly, by characters, between the end of the
/// word before (or `from_s`) and the start of the word after (or `to_s`).
pub fn interpolate(
    words: &[String],
    times: &[Option<(f64, f64)>],
    from_s: f64,
    to_s: f64,
) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(times.len());
    let mut i = 0;
    while i < times.len() {
        if let Some(t) = times[i] {
            out.push(t);
            i += 1;
            continue;
        }
        let run_end = (i..times.len())
            .find(|&j| times[j].is_some())
            .unwrap_or(times.len());
        let left = out.last().map_or(from_s, |t| t.1);
        let right = times
            .get(run_end)
            .copied()
            .flatten()
            .map_or(to_s, |t| t.0)
            .max(left);
        let weights: Vec<f64> = words[i..run_end]
            .iter()
            .map(|w| w.chars().count().max(1) as f64)
            .collect();
        let total: f64 = weights.iter().sum();
        let mut at = left;
        for w in weights {
            let next = at + (right - left) * w / total;
            out.push((at, next));
            at = next;
        }
        i = run_end;
    }
    out
}

/// One utterance under judgement: its aligned times, its recognition window `(start, end)`, and
/// the backbone's time for each displayed word.
pub type Part<'a> = (
    &'a [Option<(f64, f64)>],
    (f64, f64),
    &'a [Option<(f64, f64)>],
);

/// Whether an alignment of utterances passes: no collapsed runs, every utterance's timed speech
/// inside its window (± `WINDOW_TOLERANCE_S`), and a median start difference from the backbone
/// of at most `MAX_MEDIAN_DIFF_S`.
pub fn passes(parts: &[Part]) -> bool {
    let mut all_timed = Vec::new();
    let mut aligned_all = Vec::new();
    let mut reference_all = Vec::new();
    for (aligned, (start_s, end_s), reference) in parts {
        let timed: Vec<(f64, f64)> = aligned.iter().flatten().copied().collect();
        let (Some(first), Some(last)) = (timed.first(), timed.last()) else {
            return false;
        };
        if first.0 < start_s - WINDOW_TOLERANCE_S || last.1 > end_s + WINDOW_TOLERANCE_S {
            return false;
        }
        all_timed.extend(timed);
        for (a, r) in aligned.iter().zip(reference.iter()) {
            if let Some(r) = r {
                aligned_all.push(*a);
                reference_all.push(*r);
            }
        }
    }
    if checks::flat_runs(&all_timed) > 0 {
        return false;
    }
    let comparison = checks::compare(&aligned_all, &reference_all);
    comparison.words == 0 || comparison.median_start_diff_s <= MAX_MEDIAN_DIFF_S
}

/// The signed median of `a − b` over paired values; `None` when there are none.
pub fn signed_median(diffs: &mut [f64]) -> Option<f64> {
    if diffs.is_empty() {
        return None;
    }
    diffs.sort_by(f64::total_cmp);
    let n = diffs.len();
    Some(if n % 2 == 1 {
        diffs[n / 2]
    } else {
        (diffs[n / 2 - 1] + diffs[n / 2]) / 2.0
    })
}

#[cfg(test)]
#[path = "tests/timing.rs"]
mod tests;
