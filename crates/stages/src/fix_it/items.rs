//! The lines Fix It asks about: each quality-check finding it can fix, sorted into its problem
//! family, put in words, and the brief's suspects.
//!
//! **Role:** decide for each finding whether Fix It asks about it (`asks_about`, which the window
//! also uses to show the button), find the lines behind speech with no subtitle, and gather one
//! item per line and family with its problems in words.
//!
//! **Position:** called by `fix_it::run` after the brief, and by the window's report.
//!
//! **Signals and state:** none; pure.
//!
//! **Invariants:** a line the owner settled is never asked about; a Fix It change the owner has
//! not checked is not asked about its words again; the aligner's offset and failed calls are not
//! Fix It's; items come out by family, then in sheet order.

use std::collections::{BTreeMap, HashMap};

use job_model::outputs::{Corrections, FixBrief, FixFamily, Utterance};
use job_model::report::{QcCheck, QcFinding};

use super::Episode;
use crate::diff_sheet::sheet::clock;

/// Seconds around speech with no subtitle within which a line counts as next to it.
pub const NEAR_S: f64 = 0.5;
/// Lines asked about for one stretch of speech with no subtitle, at most.
pub const LINES_PER_GAP: usize = 3;

/// One line asked about in one family.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: String,
    pub family: FixFamily,
    pub problems: Vec<String>,
    /// Whether an answer that keeps the words settles the line: it is timed again (timing) or
    /// the model confirms a word it was asked about (unsure, a heard word replaced).
    pub keep_settles: bool,
}

/// The family Fix It asks about `finding` in, or `None` when it does not ask.
pub fn asks_about(finding: &QcFinding, corrections: &Corrections) -> Option<FixFamily> {
    use QcCheck::*;
    let family = match finding.check {
        Unsure | Novel | RemovedLocked => FixFamily::Words,
        UncoveredSpeech | WeakTiming | Overlap | GapTooSmall | TooShort | TooLong | LineTooLong
        | TooManyLines | Empty | PastEnd => FixFamily::Timing,
        TooFast => FixFamily::ReadingSpeed,
        Offset | FailedCall => return None,
    };
    if finding.check == UncoveredSpeech {
        return Some(family);
    }
    let id = finding.utterance.as_deref()?;
    match corrections.get(id) {
        Some(c) if c.by_owner() => None,
        Some(_) if family == FixFamily::Words => None,
        _ => Some(family),
    }
}

/// Every item of the run: the findings Fix It asks about, and the brief's suspects as word
/// problems.
pub fn items(ep: &Episode, brief: &FixBrief) -> Vec<Item> {
    let order: HashMap<&str, usize> = ep
        .sheet
        .iter()
        .enumerate()
        .map(|(i, u)| (u.id.as_str(), i))
        .collect();
    let mut gathered: BTreeMap<(FixFamily, usize), Item> = BTreeMap::new();
    let mut add = |id: &str, family: FixFamily, problem: String, keep_settles: bool| {
        let Some(&index) = order.get(id) else {
            return;
        };
        let item = gathered.entry((family, index)).or_insert_with(|| Item {
            id: id.to_string(),
            family,
            problems: Vec::new(),
            keep_settles: false,
        });
        if !item.problems.contains(&problem) {
            item.problems.push(problem);
        }
        item.keep_settles |= keep_settles;
    };
    for finding in &ep.qc.findings {
        let Some(family) = asks_about(finding, ep.corrections) else {
            continue;
        };
        let settles = family == FixFamily::Timing
            || matches!(finding.check, QcCheck::Unsure | QcCheck::RemovedLocked);
        if finding.check == QcCheck::UncoveredSpeech {
            let (from, to) = uncovered_span(finding);
            let problem = uncovered_problem(ep, from, to);
            for id in lines_near(ep.sheet, ep.corrections, from, to) {
                add(&id, family, problem.clone(), settles);
            }
        } else if let Some(id) = &finding.utterance {
            add(id, family, describe(finding), settles);
        }
    }
    for suspect in &brief.suspects {
        let open = ep.corrections.get(&suspect.id).is_none();
        if open {
            let problem = format!("does not fit the conversation: {}", suspect.why);
            add(&suspect.id, FixFamily::Words, problem, false);
        }
    }
    gathered.into_values().collect()
}

/// The stretch of speech with no subtitle a finding names: its start, and its start plus the
/// seconds in its detail (1 s when the detail says none).
pub fn uncovered_span(finding: &QcFinding) -> (f64, f64) {
    let seconds = finding
        .detail
        .trim()
        .trim_end_matches('s')
        .trim()
        .parse::<f64>()
        .unwrap_or(1.0);
    (finding.time_s, finding.time_s + seconds)
}

/// The lines next to `from`–`to` the owner has not settled, nearest first, at most
/// `LINES_PER_GAP`.
pub fn lines_near(
    sheet: &[Utterance],
    corrections: &Corrections,
    from: f64,
    to: f64,
) -> Vec<String> {
    let distance = |u: &Utterance| (u.start_s - to).max(from - u.end_s).max(0.0);
    let mut near: Vec<&Utterance> = sheet
        .iter()
        .filter(|u| distance(u) <= NEAR_S && !corrections.by_owner(&u.id))
        .collect();
    near.sort_by(|a, b| distance(a).total_cmp(&distance(b)));
    near.truncate(LINES_PER_GAP);
    near.into_iter().map(|u| u.id.clone()).collect()
}

/// Speech with no subtitle in words, with what the main engine heard in it.
fn uncovered_problem(ep: &Episode, from: f64, to: f64) -> String {
    let heard: Vec<&str> = ep
        .heard
        .words()
        .filter(|w| {
            let middle = (w.start_s + w.end_s) / 2.0;
            middle >= from && middle <= to
        })
        .map(|w| w.text.as_str())
        .collect();
    let said = if heard.is_empty() {
        String::new()
    } else {
        format!("; the main engine heard there: \"{}\"", heard.join(" "))
    };
    format!(
        "speech from {} to {} has no subtitle{said}",
        clock(from),
        clock(to)
    )
}

/// A finding about one line in words.
pub fn describe(finding: &QcFinding) -> String {
    let word = || {
        finding
            .detail
            .split_once(": ")
            .map_or(finding.detail.as_str(), |(_, w)| w)
            .to_string()
    };
    let cue = &finding.text;
    let detail = &finding.detail;
    match finding.check {
        QcCheck::Unsure => "the language model could not settle it".into(),
        QcCheck::Novel => format!("uses \"{}\", which no engine heard", word()),
        QcCheck::RemovedLocked => format!("leaves out \"{}\", which both engines heard", word()),
        QcCheck::WeakTiming => format!("loosely timed: {detail}"),
        QcCheck::TooShort => format!(
            "its subtitle \"{cue}\" is on screen only {detail}; it needs at least 20 frames"
        ),
        QcCheck::TooLong => format!("its subtitle \"{cue}\" stays on screen {detail}, over 7 s"),
        QcCheck::LineTooLong => {
            format!("its subtitle \"{cue}\" has a line of {detail}, over 42")
        }
        QcCheck::TooManyLines => format!("its subtitle \"{cue}\" has {detail}, over two"),
        QcCheck::Empty => "its subtitle is empty".into(),
        QcCheck::PastEnd => format!("its subtitle \"{cue}\" ends after the video"),
        QcCheck::Overlap => format!("its subtitle \"{cue}\" runs {detail}"),
        QcCheck::GapTooSmall => format!("its subtitle \"{cue}\" leaves a {detail} to the next"),
        QcCheck::TooFast => format!("its subtitle \"{cue}\" reads at {detail}; 20 is the most"),
        QcCheck::UncoveredSpeech | QcCheck::Offset | QcCheck::FailedCall => {
            finding.check.describe().to_string()
        }
    }
}

#[cfg(test)]
#[path = "tests/items.rs"]
mod tests;
