//! The automatic checks on a language model's answer: every id once, no invented words, no agreed
//! word dropped, reading speed.
//!
//! **Role:** hold a model's answer against the sheet it answers and list what breaks the rules.
//!
//! **Position:** called by the adjudication stage and the stack spike tool after the model calls.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** a word counts as heard when any engine heard it in the utterance or the ones
//! next to it; the glossary's words are always allowed.

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::diff_sheet::align;
use crate::diff_sheet::sheet::Utterance;

/// Filler words the model may drop even when every engine heard them.
pub const FILLER: [&str; 6] = ["uh", "um", "er", "erm", "uhh", "umm"];
/// Characters per second over which a line is flagged.
pub const MAX_CPS: f64 = 25.0;

/// One adjudicated utterance, as the model returns it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Line {
    pub id: String,
    pub t: String,
    #[serde(default)]
    pub f: Vec<String>,
}

/// What the checks found.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Findings {
    pub missing_ids: Vec<String>,
    pub duplicate_ids: Vec<String>,
    pub unknown_ids: Vec<String>,
    /// `(id, word)` for each output word no engine heard nearby and the glossary lacks.
    pub novel: Vec<(String, String)>,
    /// `(id, word)` for each agreed, non-filler word the answer dropped.
    pub removed_locked: Vec<(String, String)>,
    /// Ids whose text reads faster than `MAX_CPS`.
    pub too_fast: Vec<String>,
}

/// Check `lines` against the `sheet` they answer.
pub fn check(sheet: &[Utterance], lines: &[Line], glossary: &[&str]) -> Findings {
    let mut findings = Findings::default();
    let index: BTreeMap<&str, usize> = sheet
        .iter()
        .enumerate()
        .map(|(i, u)| (u.id.as_str(), i))
        .collect();
    let mut seen = HashSet::new();
    let known_glossary: HashSet<String> = glossary
        .iter()
        .flat_map(|g| g.split_whitespace())
        .map(align::normalise)
        .collect();
    for line in lines {
        let Some(&i) = index.get(line.id.as_str()) else {
            findings.unknown_ids.push(line.id.clone());
            continue;
        };
        if !seen.insert(line.id.clone()) {
            findings.duplicate_ids.push(line.id.clone());
            continue;
        }
        let utterance = &sheet[i];
        let heard: HashSet<String> = sheet[i.saturating_sub(1)..(i + 2).min(sheet.len())]
            .iter()
            .flat_map(|u| u.hypotheses.iter().flat_map(|(_, words)| words.iter()))
            .flat_map(|w| split_words(w))
            .collect();
        let said: Vec<String> = split_words(&line.t.replace("||", " "));
        // A locked word may come back joined with its neighbour (Don Quixote as Donquixote) or
        // hyphenated; it counts as kept when it still appears inside the joined words.
        let joined: String = said.concat();
        for word in &said {
            if !heard.contains(word) && !known_glossary.contains(word) {
                findings.novel.push((line.id.clone(), word.clone()));
            }
        }
        let dropped = line.f.iter().any(|f| f == "DROP" || f == "LYRIC");
        if !dropped {
            for (word, locked) in utterance.words.iter().zip(&utterance.locked) {
                let norm = align::normalise(&word.text);
                if *locked
                    && !norm.is_empty()
                    && !FILLER.contains(&norm.as_str())
                    && !said.contains(&norm)
                    && !joined.contains(&norm)
                {
                    findings
                        .removed_locked
                        .push((line.id.clone(), word.text.clone()));
                }
            }
            let seconds = (utterance.end_s - utterance.start_s).max(0.1);
            if line.t.chars().count() as f64 / seconds > MAX_CPS {
                findings.too_fast.push(line.id.clone());
            }
        }
    }
    for u in sheet {
        if !seen.contains(&u.id) {
            findings.missing_ids.push(u.id.clone());
        }
    }
    findings
}

/// Normalised words of a text, hyphenated words split as the engines may split them.
fn split_words(text: &str) -> Vec<String> {
    text.split(|c: char| c.is_whitespace() || c == '-' || c == '—')
        .map(align::normalise)
        .filter(|w| !w.is_empty())
        .collect()
}

#[cfg(test)]
#[path = "tests/checks.rs"]
mod tests;
