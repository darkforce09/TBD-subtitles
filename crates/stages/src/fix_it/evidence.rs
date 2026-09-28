//! What the model reads about one asked line: where it sits, what each engine heard, the line as
//! it stands, which words the aligner could not place, and the lines around it.
//!
//! **Role:** write one block per line for the repair and judge messages.
//!
//! **Position:** used by `repair.rs` and `judge.rs`.
//!
//! **Signals and state:** none; pure.
//!
//! **Invariants:** the block carries the utterance's start, its length and the gaps to its
//! neighbours, never a word's time; the neighbours are shown as the subtitles have them now.

use std::collections::HashMap;

use job_model::outputs::{AlignedUtterance, Line, TimingSource, Utterance};

use super::Episode;
use crate::diff_sheet::sheet::clock;

/// Lines shown on each side of an asked line.
pub const AROUND: usize = 3;

/// The lines, timing and sheet of a video, looked up by id.
pub struct Lookup<'a> {
    pub sheet: &'a [Utterance],
    lines: HashMap<&'a str, &'a Line>,
    timing: HashMap<&'a str, &'a AlignedUtterance>,
}

impl<'a> Lookup<'a> {
    pub fn new(ep: &Episode<'a>) -> Lookup<'a> {
        Lookup {
            sheet: ep.sheet,
            lines: ep.lines.iter().map(|l| (l.id.as_str(), l)).collect(),
            timing: ep
                .timing
                .utterances
                .iter()
                .map(|u| (u.id.as_str(), u))
                .collect(),
        }
    }

    /// Where the line sits: its start and length, and the gaps to the lines around it.
    pub fn place(&self, index: usize) -> String {
        let u = &self.sheet[index];
        let mut place = format!("at {}, {:.1} s long", clock(u.start_s), u.end_s - u.start_s);
        if let Some(before) = index.checked_sub(1).and_then(|i| self.sheet.get(i)) {
            place.push_str(&format!(
                ", {:.1} s after {}",
                u.start_s - before.end_s,
                before.id
            ));
        }
        if let Some(after) = self.sheet.get(index + 1) {
            place.push_str(&format!(
                ", {:.1} s before {}",
                after.start_s - u.end_s,
                after.id
            ));
        }
        place
    }

    /// What each engine heard, one line per engine tag.
    pub fn heard(&self, index: usize) -> String {
        self.sheet[index]
            .hypotheses
            .iter()
            .map(|(tag, words)| {
                let words = if words.is_empty() {
                    "(nothing)".to_string()
                } else {
                    words.join(" ")
                };
                format!("Heard {tag}: {words}\n")
            })
            .collect()
    }

    /// The words as the subtitles time them, `~` before each the aligner could not place.
    pub fn timing(&self, id: &str) -> String {
        match self.timing.get(id) {
            Some(u) if !u.words.is_empty() => u
                .words
                .iter()
                .map(|w| match w.source {
                    TimingSource::Ctc | TimingSource::CtcUtterance => w.text.clone(),
                    TimingSource::Backbone | TimingSource::Interpolated => format!("~{}", w.text),
                })
                .collect::<Vec<_>>()
                .join(" "),
            _ => "(not in the subtitles)".to_string(),
        }
    }

    /// A line as the subtitles have it now, with its flags.
    pub fn now(&self, id: &str) -> String {
        match self.lines.get(id) {
            Some(line) => shown(&line.t, &line.f),
            None => "(no line)".to_string(),
        }
    }

    /// The lines before and after, as the subtitles have them now.
    pub fn around(&self, index: usize) -> String {
        let show = |range: std::ops::Range<usize>| {
            range
                .filter_map(|i| self.sheet.get(i))
                .map(|u| format!("{}: {}", u.id, self.now(&u.id)))
                .collect::<Vec<_>>()
                .join(" · ")
        };
        let before = show(index.saturating_sub(AROUND)..index);
        let after = show(index + 1..index + 1 + AROUND);
        format!("Before it: {before}\nAfter it: {after}\n")
    }
}

/// A text with its flags in front, as a block shows it.
pub fn shown(text: &str, flags: &[String]) -> String {
    let text = if text.is_empty() { "(empty)" } else { text };
    if flags.is_empty() {
        format!("\"{text}\"")
    } else {
        format!("[{}] \"{text}\"", flags.join(" "))
    }
}

/// The block a repair reads about one line: its problems, what was heard, the line now (`text`
/// and `flags`, after earlier families), its timing and the lines around it.
pub fn repair_block(
    lookup: &Lookup,
    index: usize,
    problems: &[String],
    text: &str,
    flags: &[String],
) -> String {
    let id = &lookup.sheet[index].id;
    format!(
        "### {id} {}\nProblems: {}\n{}Now: {}\nTiming: {}\n{}",
        lookup.place(index),
        problems.join("; "),
        lookup.heard(index),
        shown(text, flags),
        lookup.timing(id),
        lookup.around(index)
    )
}

#[cfg(test)]
#[path = "tests/evidence.rs"]
mod tests;
