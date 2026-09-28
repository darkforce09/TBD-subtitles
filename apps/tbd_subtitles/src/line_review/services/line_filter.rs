//! Which lines the Check Lines list shows: the lines to check, those checked, or every line,
//! narrowed to one group and to a search by words, id or time; and the line the editor shows.
//!
//! **Role:** filter and count a review's lines, read a typed time, and find the line before or
//! after the open one.
//!
//! **Position:** called by the review view for its list and editor, by `review_editing` to move
//! on after a save, and by the application for the arrow keys and the line a clip plays.
//!
//! **Signals and state:** none; reads the session.
//!
//! **Invariants:** the list keeps sheet order; a line to check is worth a listen and not
//! corrected, a checked line is corrected; the counts ignore the group and the search; the editor
//! shows the open line while the list shows it, else the list's first; a typed time too large to
//! count is no time.

use crate::line_review::models::session::{LineList, ReviewLine, ReviewSession};

/// How many lines each list holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Counts {
    pub(crate) to_check: usize,
    pub(crate) checked: usize,
    pub(crate) all: usize,
}

/// How many lines are worth a listen: those to check and those checked.
pub(crate) fn worth(session: &ReviewSession) -> usize {
    session
        .lines
        .iter()
        .filter(|line| session.worth(line))
        .count()
}

/// The counts of the three lists, whatever the group and the search.
pub(crate) fn counts(session: &ReviewSession) -> Counts {
    let count = |list| {
        session
            .lines
            .iter()
            .filter(|line| listed(session, line, list))
            .count()
    };
    Counts {
        to_check: count(LineList::ToCheck),
        checked: count(LineList::Checked),
        all: session.lines.len(),
    }
}

/// The lines the list shows, in sheet order.
pub(crate) fn shown(session: &ReviewSession) -> Vec<&ReviewLine> {
    let query = session.search.trim().to_lowercase();
    let time = parse_time(&query);
    session
        .lines
        .iter()
        .filter(|line| listed(session, line, session.list))
        .filter(|line| session.group.is_none_or(|group| line.in_group(group)))
        .filter(|line| query.is_empty() || matches(session, line, &query, time))
        .collect()
}

/// Whether `line` belongs in `list`.
fn listed(session: &ReviewSession, line: &ReviewLine, list: LineList) -> bool {
    let corrected = session.correction(&line.id).is_some();
    match list {
        LineList::ToCheck => session.worth(line) && !corrected,
        LineList::Checked => corrected,
        LineList::All => true,
    }
}

/// Whether `line` matches the lower-case `query`: its words as shown now hold it, its id is it,
/// or it is said during the typed `time`.
fn matches(
    session: &ReviewSession,
    line: &ReviewLine,
    query: &str,
    time: Option<(f64, f64)>,
) -> bool {
    if let Some((at, step)) = time {
        return line.start_s < at + step && line.end_s >= at;
    }
    line.id.to_lowercase() == query || session.current(line).text.to_lowercase().contains(query)
}

/// A typed video time and how much of the video it names: `16:33` is the second from 16:33,
/// `16:33.4` a tenth of a second, `16:3` the ten seconds from 16:30, `16:` the minute, and
/// `1:02:05` a second past the hour. `None` when it is no time.
pub(crate) fn parse_time(query: &str) -> Option<(f64, f64)> {
    let parts: Vec<&str> = query.trim().split(':').collect();
    let (before, seconds) = match parts.as_slice() {
        [minutes, seconds] => (whole(minutes)?.checked_mul(60)?, *seconds),
        [hours, minutes, seconds] if minutes.len() == 2 => {
            let minutes = whole(minutes).filter(|m| *m < 60)?;
            let hours = whole(hours)?.checked_mul(3600)?;
            (hours.checked_add(minutes * 60)?, *seconds)
        }
        _ => return None,
    };
    let (digits, fraction) = match seconds.split_once('.') {
        Some((digits, fraction)) => (digits, Some(fraction)),
        None => (seconds, None),
    };
    let (value, step) = match (digits.len(), fraction) {
        (0, None) => (0.0, 60.0),
        (1, None) => (f64::from(whole(digits)?) * 10.0, 10.0),
        (2, None | Some("")) => (f64::from(whole(digits)?), 1.0),
        (2, Some(fraction)) => {
            whole(fraction)?;
            let places = i32::try_from(fraction.len()).ok()?;
            let value = format!("{digits}.{fraction}").parse::<f64>().ok()?;
            (value, 10f64.powi(-places))
        }
        _ => return None,
    };
    (value < 60.0).then_some((f64::from(before) + value, step))
}

/// `text` as a whole number: digits only, at least one.
fn whole(text: &str) -> Option<u32> {
    let digits = !text.is_empty() && text.chars().all(|c| c.is_ascii_digit());
    digits.then(|| text.parse().ok()).flatten()
}

/// The line the editor shows: the open line while the list shows it, else the list's first.
pub(crate) fn open_line(session: &ReviewSession) -> Option<&ReviewLine> {
    let shown = shown(session);
    let open = session.open.as_deref();
    shown
        .iter()
        .find(|line| Some(line.id.as_str()) == open)
        .or_else(|| shown.first())
        .copied()
}

/// The line after (or before) the one the editor shows, among the lines shown.
pub(crate) fn neighbour(session: &ReviewSession, forward: bool) -> Option<String> {
    let shown = shown(session);
    let open = open_line(session)?;
    let at = shown.iter().position(|line| line.id == open.id)?;
    let next = if forward {
        at.checked_add(1)
    } else {
        at.checked_sub(1)
    };
    next.and_then(|i| shown.get(i)).map(|line| line.id.clone())
}

#[cfg(test)]
#[path = "tests/line_filter.rs"]
mod tests;
