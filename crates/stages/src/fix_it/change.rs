//! The words a Fix It change touched: the first stretch of a line's words that differs between
//! before and after, as written.
//!
//! **Role:** name what a recorded change did in a few words, such as `"Uh,"` put in or
//! `"Yeah,"` taken out, so the owner reads the change without comparing two whole lines.
//!
//! **Position:** re-exported as `stages::fix_it::changed_words`, for the window.
//!
//! **Signals and state:** none; pure.
//!
//! **Invariants:** words are lined up folded, as the diff sheet folds them, so a word that only
//! changes case or punctuation beside a word put in, taken out or replaced is not part of the
//! change; a change of case or punctuation alone is still found; `None` only when the words are
//! the same.

use job_model::outputs::LineFix;

use crate::diff_sheet::align::{Step, align, normalise};

/// The first run of words that differs, before then after, each as written with its
/// punctuation: `("", "Uh,")` for a word put in, `("Yeah,", "")` for one taken out. `None` when
/// the text is the same.
pub fn changed_words(line: &LineFix) -> Option<(String, String)> {
    let before: Vec<&str> = line.before_text.split_whitespace().collect();
    let after: Vec<&str> = line.after_text.split_whitespace().collect();
    if before == after {
        return None;
    }
    let fold = |words: &[&str]| words.iter().map(|w| normalise(w)).collect::<Vec<_>>();
    let steps = align(&fold(&before), &fold(&after));
    let edited = steps.iter().any(|s| !matches!(s, Step::Match(..)));
    let differs = |step: &&Step| match **step {
        Step::Match(i, j) => !edited && before[i] != after[j],
        _ => true,
    };
    let (mut was, mut is) = (Vec::new(), Vec::new());
    for step in steps.iter().skip_while(|s| !differs(s)).take_while(differs) {
        match *step {
            Step::Match(i, j) | Step::Substitute(i, j) => {
                was.push(before[i]);
                is.push(after[j]);
            }
            Step::Delete(i) => was.push(before[i]),
            Step::Insert(j) => is.push(after[j]),
        }
    }
    Some((was.join(" "), is.join(" ")))
}

#[cfg(test)]
#[path = "tests/change.rs"]
mod tests;
