//! The rules every proposed change is held against before anything else sees it.
//!
//! **Role:** turn one repair answer into a change, a keep, or the rule it breaks: a reason given,
//! known flags only, no empty line unless dropped, no sheet notation, and no word no engine heard
//! (the adjudication's novelty check); a reading-speed fix only takes words out. Also list the
//! heard words a change leaves out, for the judge and the record.
//!
//! **Position:** called by `repair.rs` on each answered line; `removed` by `fix_it::run`.
//!
//! **Signals and state:** none; pure.
//!
//! **Invariants:** a word counts as heard when an engine heard it in the line or the ones next to
//! it, or when the glossary has it, exactly as for adjudication; the flags come back in one order,
//! with `UNSURE` never among them.

use job_model::outputs::{FixFamily, Line, Utterance};
use serde::Deserialize;

use crate::adjudication::checks;
use crate::diff_sheet::align;

/// The flags a change may set, in the order they are kept.
pub const FLAGS: [&str; 4] = ["NARR", "SPK", "LYRIC", "DROP"];

/// One line of a repair answer.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Answer {
    pub id: String,
    pub t: String,
    pub f: Vec<String>,
    pub why: String,
}

/// What an answer that keeps the rules asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// New text or flags.
    Change { text: String, flags: Vec<String> },
    /// The line as it is now.
    Keep,
}

/// `flags` in the kept order, each once, without `UNSURE`.
pub fn ordered(flags: &[String]) -> Vec<String> {
    FLAGS
        .iter()
        .filter(|flag| flags.iter().any(|f| f == *flag))
        .map(|f| f.to_string())
        .collect()
}

/// Hold `answer`, from the `family` repair, against the rules for a line that now reads `text`
/// with `flags`.
pub fn check(
    family: FixFamily,
    answer: &Answer,
    text: &str,
    flags: &[String],
    sheet: &[Utterance],
    glossary: &[&str],
) -> Result<Outcome, String> {
    if answer.why.trim().is_empty() {
        return Err("gave no reason".into());
    }
    if let Some(unknown) = answer.f.iter().find(|f| !FLAGS.contains(&f.as_str())) {
        return Err(format!("used the flag {unknown}"));
    }
    let new_text = answer.t.trim().to_string();
    let new_flags = ordered(&answer.f);
    if new_text == text && new_flags == ordered(flags) {
        return Ok(Outcome::Keep);
    }
    let dropped = new_flags.iter().any(|f| f == "DROP");
    if new_text.is_empty() && !dropped {
        return Err("left the line empty without DROP".into());
    }
    if new_text
        .replace("||", " ")
        .contains(['{', '}', '[', ']', '♪', '|'])
    {
        return Err("wrote sheet notation into the line".into());
    }
    if family == FixFamily::ReadingSpeed {
        if new_flags != ordered(flags) {
            return Err("changed the flags in a reading-speed fix".into());
        }
        if !only_takes_out(text, &new_text) {
            return Err("added or replaced a word in a reading-speed fix".into());
        }
    }
    let line = Line {
        id: answer.id.clone(),
        t: new_text.clone(),
        f: new_flags.clone(),
    };
    let novel = checks::check(sheet, &[line], glossary).novel;
    if !novel.is_empty() {
        let words: Vec<String> = novel.into_iter().map(|(_, w)| format!("\"{w}\"")).collect();
        return Err(format!("used words no engine heard: {}", words.join(", ")));
    }
    Ok(Outcome::Change {
        text: new_text,
        flags: new_flags,
    })
}

/// Whether `after` keeps only words of `before`, in their order.
pub fn only_takes_out(before: &str, after: &str) -> bool {
    let before = normalised(before);
    let mut rest = before.iter();
    normalised(after)
        .iter()
        .all(|word| rest.by_ref().any(|b| b == word))
}

/// The words of `before` that `after` no longer has, as `before` writes them.
pub fn removed(before: &str, after: &str) -> Vec<String> {
    let mut left: Vec<String> = normalised(after);
    let mut gone = Vec::new();
    for word in words(before) {
        let norm = align::normalise(word);
        if norm.is_empty() {
            continue;
        }
        match left.iter().position(|w| *w == norm) {
            Some(at) => {
                left.remove(at);
            }
            None => gone.push(word.to_string()),
        }
    }
    gone
}

/// A text's words, split as the engines may split them.
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| c.is_whitespace() || c == '-' || c == '—' || c == '|')
        .filter(|w| !w.is_empty())
}

fn normalised(text: &str) -> Vec<String> {
    words(text)
        .map(align::normalise)
        .filter(|w| !w.is_empty())
        .collect()
}

#[cfg(test)]
#[path = "tests/guard.rs"]
mod tests;
