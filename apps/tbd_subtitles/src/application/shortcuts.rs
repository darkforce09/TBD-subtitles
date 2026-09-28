//! The window's keyboard shortcuts: Ctrl+O adds videos, Ctrl+Shift+O a folder, Ctrl+, opens
//! Settings, Delete removes the selected row, and ↑ and ↓ move through the sidebar.
//!
//! **Role:** turn the shortcut keys of a frame into actions.
//!
//! **Position:** called by the application's frame before anything is drawn; the order of the
//! rows comes from the sidebar.
//!
//! **Signals and state:** consumes the keys it uses from egui's input.
//!
//! **Invariants:** Ctrl+Shift+O is matched before Ctrl+O; Delete and the arrows do nothing while
//! a text box has focus or a menu is open.

use eframe::egui::{Context, Key, Modifiers, Popup};

use super::{Action, TbdSubtitlesApp};
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::ui::row_order;

/// Push the actions of the shortcut keys pressed since the last frame onto `actions`.
pub(super) fn shortcuts(ctx: &Context, app: &TbdSubtitlesApp, actions: &mut Vec<Action>) {
    let busy = ctx.text_edit_focused() || Popup::is_any_open(ctx);
    let (folder, videos, settings, delete, step) = ctx.input_mut(|input| {
        let folder = input.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::O);
        let videos = !folder && input.consume_key(Modifiers::COMMAND, Key::O);
        let settings = input.consume_key(Modifiers::COMMAND, Key::Comma);
        if busy {
            return (folder, videos, settings, false, 0);
        }
        let delete = input.consume_key(Modifiers::NONE, Key::Delete);
        let up = input.consume_key(Modifiers::NONE, Key::ArrowUp);
        let down = input.consume_key(Modifiers::NONE, Key::ArrowDown);
        (
            folder,
            videos,
            settings,
            delete,
            i32::from(down) - i32::from(up),
        )
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
