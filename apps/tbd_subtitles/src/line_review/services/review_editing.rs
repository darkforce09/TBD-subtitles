//! The owner's edits of a review's lines: open a line, take an engine's reading or type a text,
//! set its flags, discard the edit, save it or keep the line as it is (Looks Right, or Keep Change
//! for a Fix It change) to `review.json`, undo a Fix It change, take a correction back, follow
//! each saved line's correction run, and carry the edits over when the lines are read again.
//!
//! **Role:** every change the review view can make to the session and to the corrections file.
//!
//! **Position:** called by the application's review actions; uses `line_filter` for the line the
//! editor shows and the line after it.
//!
//! **Signals and state:** changes `review.json` in the job's work directory under its lock
//! (`pipeline::work_dir::update_corrections`), so a Fix It change written meanwhile is kept, or
//! removes it when the last correction is taken back.
//!
//! **Invariants:** a line keeps its words unless the owner saves a correction or Fix It changed
//! it; Keep Change keeps Fix It's words as the owner's, and Undo Change saves the language
//! model's reading as the owner's; a correction never carries `UNSURE`; an empty text is saved
//! only for a line the owner drops; a draft lives only while it differs from what its line has
//! saved, and survives opening other lines and reading the lines again; a save moves on to the
//! next line of the list, or stays on the last; a line taken back stays open, the list showing
//! every line when its own no longer shows it; a line with no correction has nothing to take
//! back; a closed review keeps its drafts and runs.

use std::collections::BTreeMap;
use std::path::Path;

use job_model::outputs::{Chosen, Correction, Corrections};
use pipeline::work_dir::WorkDir;

use crate::line_review::models::session::{
    Draft, LineList, LineStatus, Parked, ReviewLine, ReviewSession, RunState, same_flags,
    without_unsure,
};
use crate::line_review::services::line_filter;

/// The flags the owner can set on a line.
pub(crate) const EDITABLE_FLAGS: [&str; 4] = ["SPK", "NARR", "LYRIC", "DROP"];

/// Why an empty text is not saved.
const EMPTY: &str = "The text is empty. Switch on Drop the line to remove it instead.";

/// Open line `id` in the editor.
pub(crate) fn open(session: &mut ReviewSession, id: &str) {
    if session.line(id).is_some() {
        session.open = Some(id.to_string());
    }
}

/// Put the reading tagged `tag` of the line the editor shows in its text.
pub(crate) fn pick(session: &mut ReviewSession, tag: &str) {
    let text = line_filter::open_line(session)
        .and_then(|line| line.reading(tag))
        .map(str::to_string);
    if let Some(text) = text {
        edit_text(session, text);
    }
}

/// The text of the line the editor shows, as the owner typed it.
pub(crate) fn edit_text(session: &mut ReviewSession, text: String) {
    edit(session, |draft| draft.text = text);
}

/// The flags of the line the editor shows.
pub(crate) fn set_flags(session: &mut ReviewSession, flags: Vec<String>) {
    edit(session, |draft| draft.flags = flags);
}

/// Change the draft of the line the editor shows, keeping it open; a draft that ends up as the
/// line has it saved is dropped.
fn edit(session: &mut ReviewSession, change: impl FnOnce(&mut Draft)) {
    let Some(line) = line_filter::open_line(session).cloned() else {
        return;
    };
    let mut draft = session.current(&line);
    change(&mut draft);
    session.open = Some(line.id.clone());
    if differs(&draft, &session.saved(&line)) {
        session.drafts.insert(line.id, draft);
    } else {
        session.drafts.remove(&line.id);
    }
}

fn differs(a: &Draft, b: &Draft) -> bool {
    a.text != b.text || !same_flags(&a.flags, &b.flags)
}

/// Whether line `id` has an edit not saved.
pub(crate) fn is_dirty(session: &ReviewSession, id: &str) -> bool {
    match (session.drafts.get(id), session.line(id)) {
        (Some(draft), Some(line)) => differs(draft, &session.saved(line)),
        _ => false,
    }
}

/// What `line`'s row says: edited, changed by Fix It, kept, corrected, to check, or nothing.
pub(crate) fn status(session: &ReviewSession, line: &ReviewLine) -> LineStatus {
    if is_dirty(session, &line.id) {
        LineStatus::Edited
    } else if session.unchecked_fix(&line.id) {
        LineStatus::FixIt
    } else if session.kept(line) {
        LineStatus::Kept
    } else if session.correction(&line.id).is_some() {
        LineStatus::Corrected
    } else if session.worth(line) {
        LineStatus::ToCheck
    } else {
        LineStatus::Plain
    }
}

/// Throw away the edit of the line the editor shows.
pub(crate) fn discard(session: &mut ReviewSession) {
    if let Some(id) = line_filter::open_line(session).map(|line| line.id.clone()) {
        session.drafts.remove(&id);
    }
}

/// Save the line the editor shows, as edited, as its correction and write `review.json`; the
/// next line of the list, which the editor then shows.
pub(crate) fn save(session: &mut ReviewSession) -> Result<Option<String>, String> {
    let line = line_filter::open_line(session)
        .cloned()
        .ok_or_else(|| "no line is open".to_string())?;
    let draft = session.current(&line);
    let text = collapsed(&draft.text);
    let chosen = if text == collapsed(&line.adjudicated) {
        Chosen::Engine("adjudicated".to_string())
    } else {
        line.hypotheses
            .iter()
            .find(|h| collapsed(&h.text) == text)
            .map_or(Chosen::Typed, |h| Chosen::Engine(h.tag.clone()))
    };
    let correction = Correction {
        id: line.id.clone(),
        text,
        flags: without_unsure(&draft.flags),
        chosen,
    };
    commit(session, correction)
}

/// Keep the line the editor shows as it is saved: a Fix It change the owner has not checked
/// becomes the owner's, kept as Fix It wrote it; any other line is saved unchanged as the
/// language model's reading, so the review step times it again and its warnings clear. The next
/// line of the list, which the editor then shows.
pub(crate) fn looks_right(session: &mut ReviewSession) -> Result<Option<String>, String> {
    let line = line_filter::open_line(session)
        .cloned()
        .ok_or_else(|| "no line is open".to_string())?;
    if let Some(Correction {
        chosen: Chosen::FixIt { model, why },
        text,
        flags,
        ..
    }) = session.correction(&line.id).cloned()
    {
        let kept = Correction {
            id: line.id.clone(),
            text,
            flags,
            chosen: Chosen::KeptFixIt { model, why },
        };
        return commit(session, kept);
    }
    undo_change(session)
}

/// Save the line the editor shows as the language model's reading, in place of any Fix It
/// change, so the review step times it again; the line is the owner's from then on. The next
/// line of the list, which the editor then shows.
pub(crate) fn undo_change(session: &mut ReviewSession) -> Result<Option<String>, String> {
    let line = line_filter::open_line(session)
        .cloned()
        .ok_or_else(|| "no line is open".to_string())?;
    let correction = Correction {
        id: line.id.clone(),
        text: collapsed(&line.adjudicated),
        flags: without_unsure(&line.flags),
        chosen: Chosen::Engine("adjudicated".to_string()),
    };
    commit(session, correction)
}

/// Write `correction`, drop its line's draft, mark the line saved and move on: to the line after
/// it when the list still shows it, else to the one now in its place, or the one before.
fn commit(session: &mut ReviewSession, correction: Correction) -> Result<Option<String>, String> {
    if correction.text.is_empty() && !correction.has_flag("DROP") {
        return Err(EMPTY.to_string());
    }
    let id = correction.id.clone();
    let at = place(session, &id);
    let (corrections, ()) = update(&session.work_dir, |c| c.set(correction))?;
    session.corrections = corrections;
    session.drafts.remove(&id);
    mark_saved(session, &id);
    let next = after(session, &id, at);
    session.open = next.clone();
    Ok(next)
}

/// Take back the correction of line `id` and write `review.json`; the editor stays on the line,
/// on the list of every line when its list (Checked) no longer shows it, so its run's status
/// shows. Only a search that no longer matches moves it on. Whether there was a correction to
/// take back.
pub(crate) fn revert(session: &mut ReviewSession, id: &str) -> Result<bool, String> {
    let at = place(session, id);
    let (corrections, removed) = update(&session.work_dir, |c| c.remove(id))?;
    session.corrections = corrections;
    if !removed {
        return Ok(false);
    }
    session.drafts.remove(id);
    mark_saved(session, id);
    let shown = |session: &ReviewSession| line_filter::shown(session).iter().any(|l| l.id == id);
    if !shown(session) {
        let list = std::mem::replace(&mut session.list, LineList::All);
        if !shown(session) {
            session.list = list;
            session.open = after(session, id, at);
            return Ok(true);
        }
    }
    session.open = Some(id.to_string());
    Ok(true)
}

/// Where line `id` stands in the list.
fn place(session: &ReviewSession, id: &str) -> usize {
    line_filter::shown(session)
        .iter()
        .position(|line| line.id == id)
        .unwrap_or(0)
}

/// The line to show after line `id`, which stood at `at` in the list: the next one when the list
/// still shows it (the line itself when it is the last), else the one now at `at`, or the one
/// before.
fn after(session: &ReviewSession, id: &str, at: usize) -> Option<String> {
    let shown = line_filter::shown(session);
    let pick = if let Some(here) = shown.iter().position(|line| line.id == id) {
        shown.get(here + 1).or(shown.get(here))
    } else {
        shown
            .get(at)
            .or_else(|| at.checked_sub(1).and_then(|i| shown.get(i)))
    };
    pick.map(|line| line.id.clone())
}

/// Record that line `id` was saved and waits for its correction run, as the newest run.
fn mark_saved(session: &mut ReviewSession, id: &str) {
    session.runs.retain(|(line, _)| line != id);
    session.runs.push((id.to_string(), RunState::Saved));
}

/// A correction run of the video started: every saved line, and every line a failed run left,
/// is being updated.
pub(crate) fn run_started(session: &mut ReviewSession) {
    start_runs(&mut session.runs);
}

/// The correction run ended, well when `ok`: the lines it updated are updated, or failed.
pub(crate) fn run_ended(session: &mut ReviewSession, ok: bool) {
    end_runs(&mut session.runs, ok);
}

/// `run_started` on the runs of a closed review.
pub(crate) fn start_runs(runs: &mut [(String, RunState)]) {
    for (_, state) in runs {
        if matches!(*state, RunState::Saved | RunState::Failed) {
            *state = RunState::Updating;
        }
    }
}

/// `run_ended` on the runs of a closed review.
pub(crate) fn end_runs(runs: &mut [(String, RunState)], ok: bool) {
    for (_, state) in runs {
        if *state == RunState::Updating {
            *state = if ok {
                RunState::Updated
            } else {
                RunState::Failed
            };
        }
    }
}

/// What `session` keeps while it is closed: its drafts and its runs.
pub(crate) fn park(session: ReviewSession) -> Parked {
    Parked {
        drafts: session.drafts,
        runs: session.runs,
    }
}

/// Put what a closed review kept back into `session`, its lines read again.
pub(crate) fn unpark(session: &mut ReviewSession, parked: Parked) {
    session.runs = parked.runs;
    restore_drafts(session, parked.drafts);
}

/// Carry what the owner did in `old` over to `fresh`, the same lines read again: the open line,
/// the list, the search, the group, the runs, and the drafts still worth keeping.
pub(crate) fn carry_over(old: &ReviewSession, fresh: &mut ReviewSession) {
    fresh.open.clone_from(&old.open);
    fresh.list = old.list;
    fresh.search.clone_from(&old.search);
    fresh.group = old.group;
    fresh.runs.clone_from(&old.runs);
    restore_drafts(fresh, old.drafts.clone());
}

/// Put `drafts` back into `session`, leaving out those of lines it no longer has and those that
/// match what their line has saved now.
fn restore_drafts(session: &mut ReviewSession, drafts: BTreeMap<String, Draft>) {
    for (id, draft) in drafts {
        let keep = session
            .line(&id)
            .is_some_and(|line| differs(&draft, &session.saved(line)));
        if keep {
            session.drafts.insert(id, draft);
        }
    }
}

/// `text` with its runs of white space made one space, and none at its ends.
fn collapsed(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Change the corrections in `work_dir` with `change`, as the file holds them now and under its
/// lock, so a change Fix It wrote meanwhile is kept; the corrections as they now are, and what
/// `change` returned.
fn update<R>(
    work_dir: &Path,
    change: impl FnOnce(&mut Corrections) -> R,
) -> Result<(Corrections, R), String> {
    pipeline::work_dir::update_corrections(&WorkDir::new(work_dir), change)
        .map_err(|e| e.to_string())
}

/// Mark `ids`, which Fix It changed, as saved and waiting for their correction run, in the runs
/// of an open or a closed review.
pub(crate) fn mark_fixed(runs: &mut Vec<(String, RunState)>, ids: &[String]) {
    for id in ids {
        runs.retain(|(line, _)| line != id);
        runs.push((id.clone(), RunState::Saved));
    }
}

#[cfg(test)]
#[path = "tests/review_editing.rs"]
mod tests;
