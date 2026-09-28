//! Fix It: a stronger model fixes the lines the quality check flagged, in three passes, and every
//! change is held against what the speech engines heard.
//!
//! **Role:** read the whole video once for its context (the brief: show, episode, cast, scenes,
//! speech habits, lines that do not fit), fix the flagged lines one problem family at a time
//! (words, then timing and layout, then reading speed), guard each proposal, have a judge accept
//! or turn down each change against the line before it, and say what became of every line.
//!
//! **Position:** called by `pipeline::fix_it` with any `inference::llm::LanguageModel`; the
//! passes are `brief.rs`, `repair.rs` and `judge.rs`, the lines asked about come from `items.rs`,
//! what the model reads about each from `evidence.rs`, and `guard.rs` holds the rules.
//!
//! **Signals and state:** one model call per brief part and per batch; the working copy of each
//! asked line lives only for the run. The model sees each utterance's start, its length and the
//! gaps around it, never a word's time.
//!
//! **Invariants:** no word no engine heard reaches a change (law 8); a line the owner settled is
//! never asked about; a change is kept only when the judge accepts it; a line whose words stay
//! is kept only to be timed again, or when the model confirms a word it was asked about; once
//! `stop` is set no call starts and the run ends as stopped.

pub mod brief;
pub mod calls;
pub mod evidence;
pub mod guard;
pub mod items;
pub mod judge;
pub mod prompt;
pub mod repair;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use job_model::outputs::{
    Aligned, Corrections, EngineTranscript, FixBrief, FixFamily, FixStep, FixVerdict, Line,
    LineFix, Utterance,
};
use job_model::report::QcReport;

pub use calls::{Make, Usage};

/// Everything Fix It reads about one video.
pub struct Episode<'a> {
    /// The video's file name without its extension, such as `[Muhn Pace] Dressrosa 12`.
    pub video_name: &'a str,
    /// The name of the folder the video is in, such as `one_pace`.
    pub folder_name: &'a str,
    /// The glossary's name, such as `one_piece`, and its terms.
    pub glossary_name: &'a str,
    pub glossary: &'a [&'a str],
    /// The diff sheet with the re-decoded alternatives.
    pub sheet: &'a [Utterance],
    /// Every line as the subtitles have it now: the adjudication with the corrections in place.
    pub lines: &'a [Line],
    pub corrections: &'a Corrections,
    pub qc: &'a QcReport,
    /// The words as the subtitles time them now (`reviewed.json`).
    pub timing: &'a Aligned,
    /// The main engine's words, to tell what was heard where speech has no subtitle.
    pub heard: &'a EngineTranscript,
}

/// Which pass is running, for progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pass {
    Reading,
    Fixing(FixFamily),
    Checking,
}

/// Why a run ended without a result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixFailure {
    /// `stop` was set.
    Stopped,
    /// The brief could not be made, so nothing was asked.
    Brief(String),
}

/// What one run left: the brief, every asked line in sheet order, and what the calls cost.
#[derive(Debug, Clone, Default)]
pub struct FixRun {
    pub brief: FixBrief,
    pub lines: Vec<LineFix>,
    pub usage: Usage,
}

/// One asked line while the passes run.
#[derive(Debug, Clone)]
pub(crate) struct Draft {
    pub(crate) id: String,
    /// Its index in the sheet.
    pub(crate) index: usize,
    pub(crate) problems: Vec<String>,
    pub(crate) before_text: String,
    pub(crate) before_flags: Vec<String>,
    pub(crate) text: String,
    pub(crate) flags: Vec<String>,
    pub(crate) steps: Vec<FixStep>,
    pub(crate) refused: Vec<String>,
    /// Why the line may be kept as it is: timed again, or its words confirmed.
    pub(crate) kept: Option<String>,
}

impl Draft {
    pub(crate) fn changed(&self) -> bool {
        self.text != self.before_text || self.flags != self.before_flags
    }
}

/// Run the three passes over `ep` with models from `make`, `workers` calls at once. `progress`
/// hears the pass and its `(done, total)` calls.
pub fn run(
    ep: &Episode,
    make: &Make<'_>,
    workers: usize,
    progress: &(dyn Fn(Pass, usize, usize) + Sync),
    stop: &AtomicBool,
) -> Result<FixRun, FixFailure> {
    let calls = calls::Calls::new(make, workers, stop);
    let brief = brief::brief(ep, &calls, &|done, total| {
        progress(Pass::Reading, done, total)
    })?;
    let context = brief::context(ep, &brief);
    let asked = items::items(ep, &brief);
    let mut drafts = drafts(ep, &asked);
    for family in FixFamily::ALL {
        let family_items: Vec<&items::Item> = asked.iter().filter(|i| i.family == family).collect();
        repair::repair(
            ep,
            &context,
            family,
            &family_items,
            &mut drafts,
            &calls,
            &|done, total| progress(Pass::Fixing(family), done, total),
        );
        if stop.load(Ordering::SeqCst) {
            return Err(FixFailure::Stopped);
        }
    }
    let verdicts = judge::judge(ep, &context, &drafts, &calls, &|done, total| {
        progress(Pass::Checking, done, total)
    });
    if stop.load(Ordering::SeqCst) {
        return Err(FixFailure::Stopped);
    }
    let mut lines: Vec<(LineFix, usize)> = drafts
        .into_values()
        .map(|draft| finish(draft, &verdicts))
        .collect();
    lines.sort_by_key(|l| l.1);
    Ok(FixRun {
        brief,
        lines: lines.into_iter().map(|(line, _)| line).collect(),
        usage: calls.into_usage(),
    })
}

/// A draft of every asked line, its problems gathered from each family.
fn drafts(ep: &Episode, asked: &[items::Item]) -> HashMap<String, Draft> {
    let index: HashMap<&str, usize> = ep
        .sheet
        .iter()
        .enumerate()
        .map(|(i, u)| (u.id.as_str(), i))
        .collect();
    let now: HashMap<&str, &Line> = ep.lines.iter().map(|l| (l.id.as_str(), l)).collect();
    let mut drafts: HashMap<String, Draft> = HashMap::new();
    for item in asked {
        let Some(&i) = index.get(item.id.as_str()) else {
            continue;
        };
        let draft = drafts.entry(item.id.clone()).or_insert_with(|| {
            let (text, flags) = now
                .get(item.id.as_str())
                .map_or((String::new(), Vec::new()), |l| {
                    (l.t.clone(), guard::ordered(&l.f))
                });
            Draft {
                id: item.id.clone(),
                index: i,
                problems: Vec::new(),
                before_text: text.clone(),
                before_flags: flags.clone(),
                text,
                flags,
                steps: Vec::new(),
                refused: Vec::new(),
                kept: None,
            }
        });
        for problem in &item.problems {
            if !draft.problems.contains(problem) {
                draft.problems.push(problem.clone());
            }
        }
    }
    drafts
}

/// The line as it came out, with its sheet index: a changed line takes the judge's verdict; a
/// line whose words stay is kept when it may be, else unchanged.
fn finish(draft: Draft, verdicts: &HashMap<String, judge::Verdict>) -> (LineFix, usize) {
    let verdict = if draft.changed() {
        match verdicts.get(&draft.id) {
            Some(judge::Verdict::Accept(why)) => FixVerdict::Accepted { why: why.clone() },
            Some(judge::Verdict::TurnDown(why)) => FixVerdict::TurnedDown { why: why.clone() },
            None => FixVerdict::NotJudged {
                why: "the judge gave no verdict".into(),
            },
        }
    } else {
        match &draft.kept {
            Some(why) => FixVerdict::Kept { why: why.clone() },
            None => FixVerdict::Unchanged,
        }
    };
    let removed = guard::removed(&draft.before_text, &draft.text);
    let index = draft.index;
    let line = LineFix {
        id: draft.id,
        problems: draft.problems,
        before_text: draft.before_text,
        before_flags: draft.before_flags,
        after_text: draft.text,
        after_flags: draft.flags,
        steps: draft.steps,
        refused: draft.refused,
        removed,
        verdict,
        applied: false,
    };
    (line, index)
}

#[cfg(test)]
#[path = "tests/fake_model.rs"]
mod fake_model;

#[cfg(test)]
#[path = "tests/fix_it.rs"]
mod tests;
