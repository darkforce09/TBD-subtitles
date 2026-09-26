//! Measures of an alignment: how far it lies from reference times, and the signature of a silent
//! aligner failure.

/// An alignment set against reference times for the same words.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Comparison {
    /// Words timed by both.
    pub words: usize,
    /// Median of |aligned start − reference start|, in seconds.
    pub median_start_diff_s: f64,
    /// Share of words whose start differs by more than 200 ms.
    pub share_over_200ms: f64,
}

/// Compare aligned with reference `(start, end)` pairs, word by word; `None`s are skipped.
pub fn compare(aligned: &[Option<(f64, f64)>], reference: &[(f64, f64)]) -> Comparison {
    let mut diffs: Vec<f64> = aligned
        .iter()
        .zip(reference)
        .filter_map(|(a, r)| a.map(|a| (a.0 - r.0).abs()))
        .collect();
    if diffs.is_empty() {
        return Comparison::default();
    }
    diffs.sort_by(f64::total_cmp);
    Comparison {
        words: diffs.len(),
        median_start_diff_s: diffs[diffs.len() / 2],
        share_over_200ms: diffs.iter().filter(|d| **d > 0.2).count() as f64 / diffs.len() as f64,
    }
}

/// Runs of three consecutive zero-length words: words an aligner collapsed.
pub fn flat_runs(times: &[(f64, f64)]) -> usize {
    let flat = |i: usize| times[i].1 - times[i].0 < 1e-3;
    let mut runs = 0;
    let mut i = 2;
    while i < times.len() {
        if flat(i - 2) && flat(i - 1) && flat(i) {
            runs += 1;
            i += 3;
        } else {
            i += 1;
        }
    }
    runs
}

/// Runs of three or more consecutive words that are zero-length or evenly spaced with equal
/// lengths: what an aligner leaves when it gave up and spread the words out. On a coarse frame
/// grid (80 ms) short words meet the even rule by chance, so read it with `flat_runs`.
pub fn suspicious_runs(times: &[(f64, f64)]) -> usize {
    let flat = |i: usize| times[i].1 - times[i].0 < 1e-3;
    let even = |i: usize| {
        let (a, b, c) = (times[i - 2], times[i - 1], times[i]);
        let same_length =
            ((b.1 - b.0) - (a.1 - a.0)).abs() < 1e-3 && ((c.1 - c.0) - (b.1 - b.0)).abs() < 1e-3;
        let same_step = ((b.0 - a.0) - (c.0 - b.0)).abs() < 1e-3;
        same_length && same_step
    };
    let mut runs = 0;
    let mut i = 2;
    while i < times.len() {
        if (flat(i - 2) && flat(i - 1) && flat(i)) || even(i) {
            runs += 1;
            i += 3;
        } else {
            i += 1;
        }
    }
    runs
}

#[cfg(test)]
#[path = "tests/checks.rs"]
mod tests;
