//! The owner's edit of one line: open it, take an engine's reading or type a text, set its flags,
//! save it to `review.json`, or take the correction back.
//!
//! **Role:** every change the review view can make to the session and to the corrections file.
//!
//! **Position:** called by the application's review actions.
//!
//! **Signals and state:** writes `review.json` in the job's work directory (through a part file),
//! or removes it when the last correction is taken back.
//!
//! **Invariants:** a line keeps its words unless the owner saves a correction; a correction never
//! carries `UNSURE`; an empty text is saved only for a line the owner drops.

use std::path::Path;

use job_model::outputs::{Chosen, Correction, Corrections};

use crate::line_review::models::session::{Draft, ReviewSession};

/// The flags the owner can set on a line.
pub(crate) const EDITABLE_FLAGS: [&str; 4] = ["SPK", "NARR", "LYRIC", "DROP"];

/// Start editing line `id`: its correction when there is one, else the language model's text
/// and flags without `UNSURE`.
pub(crate) fn open(session: &mut ReviewSession, id: &str) {
    let Some(line) = session.line(id) else {
        return;
    };
    let draft = match session.correction(id) {
        Some(c) => Draft {
            id: id.to_string(),
            text: c.text.clone(),
            flags: c.flags.clone(),
        },
        None => Draft {
            id: id.to_string(),
            text: line.adjudicated.clone(),
            flags: line
                .flags
                .iter()
                .filter(|f| f.as_str() != "UNSURE")
                .cloned()
                .collect(),
        },
    };
    session.draft = Some(draft);
    session.notice = None;
}

/// Put the reading tagged `tag` in the draft.
pub(crate) fn pick(session: &mut ReviewSession, tag: &str) {
    let Some(draft) = &session.draft else {
        return;
    };
    let text = session.line(&draft.id).and_then(|line| {
        if tag == "adjudicated" {
            Some(line.adjudicated.clone())
        } else {
            line.hypotheses
                .iter()
                .find(|h| h.tag == tag)
                .map(|h| h.text.clone())
        }
    });
    if let (Some(text), Some(draft)) = (text, &mut session.draft) {
        draft.text = text;
    }
}

/// Save the draft as the line's correction and write `review.json`.
pub(crate) fn save(session: &mut ReviewSession) -> Result<(), String> {
    let Some(draft) = session.draft.clone() else {
        return Err("no line is open".to_string());
    };
    let text = draft.text.split_whitespace().collect::<Vec<_>>().join(" ");
    let dropped = draft.flags.iter().any(|f| f == "DROP");
    if text.is_empty() && !dropped {
        return Err("the text is empty; drop the line instead".to_string());
    }
    let line = session
        .line(&draft.id)
        .ok_or_else(|| format!("no line {}", draft.id))?;
    let chosen = if text == line.adjudicated {
        Chosen::Engine("adjudicated".to_string())
    } else {
        line.hypotheses
            .iter()
            .find(|h| h.text == text)
            .map_or(Chosen::Typed, |h| Chosen::Engine(h.tag.clone()))
    };
    let mut corrections = session.corrections.clone();
    corrections.set(Correction {
        id: draft.id.clone(),
        text,
        flags: draft
            .flags
            .iter()
            .filter(|f| f.as_str() != "UNSURE")
            .cloned()
            .collect(),
        chosen,
    });
    write(&session.work_dir, &corrections)?;
    session.corrections = corrections;
    session.notice = Some(format!(
        "{} saved; it is timed again in the queue.",
        draft.id
    ));
    Ok(())
}

/// Take back the correction of line `id` and write `review.json`.
pub(crate) fn revert(session: &mut ReviewSession, id: &str) -> Result<(), String> {
    let mut corrections = session.corrections.clone();
    if !corrections.remove(id) {
        return Ok(());
    }
    write(&session.work_dir, &corrections)?;
    session.corrections = corrections;
    open(session, id);
    session.notice = Some(format!("{id} is back to the language model's text."));
    Ok(())
}

/// The line after (or before) `id` among the lines shown.
pub(crate) fn neighbour(session: &ReviewSession, id: &str, forward: bool) -> Option<String> {
    let shown: Vec<&str> = session.shown().map(|line| line.id.as_str()).collect();
    let at = shown.iter().position(|shown| *shown == id)?;
    let next = if forward {
        at.checked_add(1)
    } else {
        at.checked_sub(1)
    };
    next.and_then(|i| shown.get(i)).map(|id| id.to_string())
}

/// Write `corrections` to `review.json` whole, or remove the file when there are none.
fn write(work_dir: &Path, corrections: &Corrections) -> Result<(), String> {
    let path = work_dir.join("review.json");
    if corrections.lines.is_empty() {
        return match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("cannot remove {}: {e}", path.display())),
        };
    }
    let text = serde_json::to_string_pretty(corrections).map_err(|e| e.to_string())?;
    let part = work_dir.join("review.json.part");
    std::fs::write(&part, text).map_err(|e| format!("cannot write {}: {e}", part.display()))?;
    std::fs::rename(&part, &path).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

#[cfg(test)]
#[path = "tests/review_editing.rs"]
mod tests;
