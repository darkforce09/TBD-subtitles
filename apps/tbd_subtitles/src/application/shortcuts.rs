//! The window's keyboard shortcuts: Ctrl+O adds videos, Ctrl+Shift+O a folder, Ctrl+, opens
//! Settings, Ctrl+L the log window, Delete removes the selected row, and ↑ and ↓ move through the
//! sidebar; in Check Lines, Ctrl+S saves an edited line, Ctrl+Enter keeps an unedited one, ↑ and ↓
//! move through the lines, Space plays or stops the clip, and Esc leaves the text box, then stops
//! the clip.
//!
//! **Role:** turn the shortcut keys of a frame into actions.
//!
//! **Position:** called by the application's frame before anything is drawn; the order of the
//! rows comes from the sidebar, the open line from `line_filter`.
//!
//! **Signals and state:** consumes the keys it uses from egui's input; Esc in a text box takes
//! the focus from it.
//!
//! **Invariants:** Ctrl+Shift+O is matched before Ctrl+O; Delete, Space and the arrows do nothing
//! while a text box has focus or a menu is open; in Check Lines the arrows move through the lines,
//! not the sidebar, and Delete removes nothing; Space is left to a widget that has the keyboard's
//! focus; nothing is saved while a full run of the video runs.

use eframe::egui::{Context, Key, Modifiers, Popup};

use super::{Action, TbdSubtitlesApp};
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::ui::row_order;
use crate::line_review::events::ReviewEvent;
use crate::line_review::models::clip::Sound;
use crate::line_review::services::{line_filter, review_editing};

/// Push the actions of the shortcut keys pressed since the last frame onto `actions`.
pub(super) fn shortcuts(ctx: &Context, app: &TbdSubtitlesApp, actions: &mut Vec<Action>) {
    let typing = ctx.text_edit_focused();
    let busy = typing || Popup::is_any_open(ctx);
    let (folder, videos, settings, log) = ctx.input_mut(|input| {
        let folder = input.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::O);
        let videos = !folder && input.consume_key(Modifiers::COMMAND, Key::O);
        let settings = input.consume_key(Modifiers::COMMAND, Key::Comma);
        let log = input.consume_key(Modifiers::COMMAND, Key::L);
        (folder, videos, settings, log)
    });
    if folder {
        actions.push(Action::Queue(JobQueueEvent::AddFolder));
    }
    if videos {
        actions.push(Action::Queue(JobQueueEvent::AddVideos));
    }
    if settings {
        actions.push(Action::ShowSettings(true));
    }
    if log {
        actions.push(Action::ShowLog(true));
    }
    if review_keys(ctx, app, typing, busy, actions) || busy {
        return;
    }
    let (delete, step) = ctx.input_mut(|input| {
        let delete = input.consume_key(Modifiers::NONE, Key::Delete);
        let up = input.consume_key(Modifiers::NONE, Key::ArrowUp);
        let down = input.consume_key(Modifiers::NONE, Key::ArrowDown);
        (delete, i32::from(down) - i32::from(up))
    });
    if delete && let Some(id) = app.queue.selected {
        actions.push(Action::Queue(JobQueueEvent::Remove(id)));
    }
    if step != 0 {
        let order = row_order(ctx, &app.queue);
        let at = app
            .queue
            .selected
            .and_then(|id| order.iter().position(|&row| row == id));
        let next = match at {
            None => 0,
            Some(at) if step > 0 => (at + 1).min(order.len().saturating_sub(1)),
            Some(at) => at.saturating_sub(1),
        };
        if let Some(&id) = order.get(next)
            && app.queue.selected != Some(id)
        {
            actions.push(Action::Queue(JobQueueEvent::Select(id)));
        }
    }
}

/// The keys of the selected job's Check Lines, when it is open; whether it is.
fn review_keys(
    ctx: &Context,
    app: &TbdSubtitlesApp,
    typing: bool,
    busy: bool,
    actions: &mut Vec<Action>,
) -> bool {
    let Some((job, session)) = app
        .review
        .as_ref()
        .filter(|(job, _)| app.queue.selected == Some(*job))
    else {
        return false;
    };
    let line = line_filter::open_line(session);
    let savable = line.is_some() && !app.video_busy(*job);
    let edited = line.is_some_and(|line| review_editing::is_dirty(session, &line.id));
    let corrected = line.is_some_and(|line| session.correction(&line.id).is_some());
    let playing = app.clip.as_ref().is_some_and(|clip| clip.is_playing());
    // A button or a switch with the keyboard's focus takes Space for itself.
    let focused = ctx.memory(|memory| memory.focused().is_some());
    let mut events = Vec::new();
    ctx.input_mut(|input| {
        if savable && edited && input.consume_key(Modifiers::COMMAND, Key::S) {
            events.push(ReviewEvent::Save);
        }
        if savable && !edited && !corrected && input.consume_key(Modifiers::COMMAND, Key::Enter) {
            events.push(ReviewEvent::LooksRight);
        }
    });
    if typing {
        if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
            ctx.memory_mut(|memory| {
                if let Some(id) = memory.focused() {
                    memory.surrender_focus(id);
                }
            });
        }
    } else if !busy {
        ctx.input_mut(|input| {
            if playing && input.consume_key(Modifiers::NONE, Key::Escape) {
                events.push(ReviewEvent::Stop);
            }
            if line.is_some() && !focused && input.consume_key(Modifiers::NONE, Key::Space) {
                events.push(if playing {
                    ReviewEvent::Stop
                } else {
                    ReviewEvent::Play(Sound::Mix)
                });
            }
            if input.consume_key(Modifiers::NONE, Key::ArrowUp) {
                events.push(ReviewEvent::Step { forward: false });
            }
            if input.consume_key(Modifiers::NONE, Key::ArrowDown) {
                events.push(ReviewEvent::Step { forward: true });
            }
        });
    }
    actions.extend(events.into_iter().map(Action::from));
    true
}
