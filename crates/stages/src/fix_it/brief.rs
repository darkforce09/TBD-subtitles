//! The first pass: the model works out the video's context by itself, from the video's file and
//! folder names, the glossary and every line, and lists the lines that do not fit.
//!
//! **Role:** write every line as the brief reads it, ask for the brief (in parts of at most 800
//! lines, each part with the summaries before it), merge the parts, and write the brief as the
//! later passes read it.
//!
//! **Position:** the first call of `fix_it::run`; its text opens every repair and judge message.
//!
//! **Signals and state:** one call per part, in order.
//!
//! **Invariants:** the owner types nothing: the context comes only from what the job already
//! holds and what the model knows of the series; a part that fails ends the run before any line
//! is asked about; suspects name lines of the sheet only, at most `MAX_SUSPECTS`.

use std::collections::HashSet;

use job_model::outputs::{FixBrief, Suspect};

use super::calls::Calls;
use super::{Episode, FixFailure, prompt};
use crate::diff_sheet::sheet::clock;

/// Lines per brief call.
pub const PART_LINES: usize = 800;
/// Suspects asked about at most.
pub const MAX_SUSPECTS: usize = 30;

/// Every line as the brief reads it: `ID m:ss.d [FLAGS] | text`.
pub fn line_list(ep: &Episode) -> Vec<String> {
    let starts: std::collections::HashMap<&str, f64> = ep
        .sheet
        .iter()
        .map(|u| (u.id.as_str(), u.start_s))
        .collect();
    ep.lines
        .iter()
        .map(|l| {
            let at = starts.get(l.id.as_str()).copied().unwrap_or(0.0);
            let flags = if l.f.is_empty() {
                String::new()
            } else {
                format!(" [{}]", l.f.join(" "))
            };
            format!("{} {}{flags} | {}", l.id, clock(at), l.t)
        })
        .collect()
}

/// The message for one part: the names, the glossary, the earlier parts' summaries, the lines.
pub fn message(ep: &Episode, part: &[String], earlier: &[String]) -> String {
    let mut text = format!(
        "Video file: {}\nFolder: {}\nGlossary ({}): {}\n",
        ep.video_name,
        ep.folder_name,
        ep.glossary_name,
        ep.glossary.join(", ")
    );
    for (i, summary) in earlier.iter().enumerate() {
        text.push_str(&format!("\nSummary of part {}: {summary}\n", i + 1));
    }
    text.push_str("\nLines:\n");
    for line in part {
        text.push_str(line);
        text.push('\n');
    }
    text
}

/// Ask for the brief; `progress` hears `(parts done, parts)`.
pub fn brief(
    ep: &Episode,
    calls: &Calls,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<FixBrief, FixFailure> {
    let lines = line_list(ep);
    let parts: Vec<&[String]> = lines.chunks(PART_LINES).collect();
    let system = if parts.len() > 1 {
        format!("{}{}", prompt::BRIEF, prompt::BRIEF_PART)
    } else {
        prompt::BRIEF.to_string()
    };
    let mut briefs: Vec<FixBrief> = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        if calls.stopped() {
            return Err(FixFailure::Stopped);
        }
        let earlier: Vec<String> = briefs.iter().map(|b| b.summary.clone()).collect();
        let label = format!("brief part {} of {}", i + 1, parts.len());
        let answer = calls
            .ask_all(
                &label,
                &system,
                &prompt::brief_schema(),
                &[message(ep, part, &earlier)],
                &|_, _| {},
            )
            .pop()
            .flatten()
            .and_then(|value| calls.read::<FixBrief>(&label, value));
        match answer {
            Some(brief) => briefs.push(brief),
            None if calls.stopped() => return Err(FixFailure::Stopped),
            None => {
                let why = calls
                    .last_failure()
                    .unwrap_or_else(|| format!("the {label} failed"));
                return Err(FixFailure::Brief(why));
            }
        }
        progress(i + 1, parts.len());
    }
    Ok(merge(ep, briefs))
}

/// One brief from the parts': the first part's show and episode, every name and habit once, the
/// summaries in order, and the suspects that name a line of the sheet.
pub fn merge(ep: &Episode, parts: Vec<FixBrief>) -> FixBrief {
    let known: HashSet<&str> = ep.sheet.iter().map(|u| u.id.as_str()).collect();
    let mut merged = FixBrief::default();
    let mut summaries = Vec::new();
    for part in parts {
        if merged.show.is_empty() {
            merged.show = part.show;
        }
        if merged.episode.is_empty() {
            merged.episode = part.episode;
        }
        for name in part.cast {
            if !merged.cast.contains(&name) {
                merged.cast.push(name);
            }
        }
        for habit in part.speech_habits {
            if !merged.speech_habits.contains(&habit) {
                merged.speech_habits.push(habit);
            }
        }
        summaries.push(part.summary);
        for suspect in part.suspects {
            let new = !merged.suspects.iter().any(|s: &Suspect| s.id == suspect.id);
            if new && known.contains(suspect.id.as_str()) {
                merged.suspects.push(suspect);
            }
        }
    }
    merged.summary = summaries.join("\n\n");
    merged.suspects.truncate(MAX_SUSPECTS);
    merged
}

/// The brief as every later message opens with it.
pub fn context(ep: &Episode, brief: &FixBrief) -> String {
    let habits = if brief.speech_habits.is_empty() {
        "none noted".to_string()
    } else {
        brief.speech_habits.join("; ")
    };
    format!(
        "Brief\nVideo file: {} (folder {})\nShow: {}\nEpisode: {}\nCast: {}\nSpeech habits: {}\n\
         Scenes: {}\nGlossary ({}): {}\n",
        ep.video_name,
        ep.folder_name,
        brief.show,
        brief.episode,
        brief.cast.join(", "),
        habits,
        brief.summary,
        ep.glossary_name,
        ep.glossary.join(", ")
    )
}

#[cfg(test)]
#[path = "tests/brief.rs"]
mod tests;
