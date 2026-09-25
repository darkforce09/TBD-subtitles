//! Word-level edit-distance alignment of two hypotheses, on normalised words.
//!
//! **Role:** fold case, punctuation and number forms so "Birdcage," and "birdcage" match, then
//! align two word lists with the fewest substitutions, deletions and insertions, and count them.
//!
//! **Position:** used by the diff sheet to line other engines up with the backbone, and by the
//! stack spike tool to measure how far two engines disagree.
//!
//! **Signals and state:** none; pure functions over word lists.
//!
//! **Invariants:** the alignment visits every word of both lists exactly once, in order; a tie
//! prefers a match, then a substitution, so equal words line up.

/// One step of an alignment: indices into the reference `a` and the hypothesis `b`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Match(usize, usize),
    Substitute(usize, usize),
    /// A word of `a` that `b` lacks.
    Delete(usize),
    /// A word of `b` that `a` lacks.
    Insert(usize),
}

/// Error counts of `b` against the reference `a`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Errors {
    pub reference_words: usize,
    pub substitutions: usize,
    pub deletions: usize,
    pub insertions: usize,
}

impl Errors {
    /// Word error rate: all errors over the reference length.
    pub fn rate(&self) -> f64 {
        if self.reference_words == 0 {
            return 0.0;
        }
        (self.substitutions + self.deletions + self.insertions) as f64 / self.reference_words as f64
    }

    pub fn add(&mut self, other: Errors) {
        self.reference_words += other.reference_words;
        self.substitutions += other.substitutions;
        self.deletions += other.deletions;
        self.insertions += other.insertions;
    }
}

/// A word folded for comparison: lowercase, letters, digits and inner apostrophes only, and
/// number words written as digits.
pub fn normalise(word: &str) -> String {
    let folded: String = word
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '\'' || *c == '’')
        .map(|c| if c == '’' { '\'' } else { c })
        .flat_map(char::to_lowercase)
        .collect();
    let folded = folded.trim_matches('\'').to_string();
    number_word(&folded).map(str::to_string).unwrap_or(folded)
}

fn number_word(word: &str) -> Option<&'static str> {
    const WORDS: [&str; 21] = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
        "twenty",
    ];
    const DIGITS: [&str; 21] = [
        "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16",
        "17", "18", "19", "20",
    ];
    WORDS.iter().position(|w| *w == word).map(|i| DIGITS[i])
}

/// Normalise every word and drop the ones that fold to nothing (stray punctuation).
pub fn normalise_all<'a>(words: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    words
        .into_iter()
        .map(normalise)
        .filter(|w| !w.is_empty())
        .collect()
}

/// The cheapest alignment of `b` against `a`.
pub fn align(a: &[String], b: &[String]) -> Vec<Step> {
    let (n, m) = (a.len(), b.len());
    let width = m + 1;
    let mut cost = vec![0u32; (n + 1) * width];
    for i in 0..=n {
        cost[i * width] = i as u32;
    }
    for (j, slot) in cost.iter_mut().enumerate().take(width) {
        *slot = j as u32;
    }
    for i in 1..=n {
        for j in 1..=m {
            let diagonal = cost[(i - 1) * width + j - 1] + u32::from(a[i - 1] != b[j - 1]);
            let up = cost[(i - 1) * width + j] + 1;
            let left = cost[i * width + j - 1] + 1;
            cost[i * width + j] = diagonal.min(up).min(left);
        }
    }
    let mut steps = Vec::with_capacity(n.max(m));
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        let here = cost[i * width + j];
        if i > 0 && j > 0 {
            let same = a[i - 1] == b[j - 1];
            if here == cost[(i - 1) * width + j - 1] + u32::from(!same) {
                steps.push(if same {
                    Step::Match(i - 1, j - 1)
                } else {
                    Step::Substitute(i - 1, j - 1)
                });
                i -= 1;
                j -= 1;
                continue;
            }
        }
        if i > 0 && here == cost[(i - 1) * width + j] + 1 {
            steps.push(Step::Delete(i - 1));
            i -= 1;
        } else {
            steps.push(Step::Insert(j - 1));
            j -= 1;
        }
    }
    steps.reverse();
    steps
}

/// Count the errors of `b` against `a`.
pub fn errors(a: &[String], b: &[String]) -> Errors {
    let mut counts = Errors {
        reference_words: a.len(),
        ..Errors::default()
    };
    for step in align(a, b) {
        match step {
            Step::Match(..) => {}
            Step::Substitute(..) => counts.substitutions += 1,
            Step::Delete(_) => counts.deletions += 1,
            Step::Insert(_) => counts.insertions += 1,
        }
    }
    counts
}

#[cfg(test)]
#[path = "tests/align.rs"]
mod tests;
