//! The log window: a second native window, beside the main window's right edge when it opens,
//! that shows every line logged while the app runs.
//!
//! **Role:** show the log in its own window while it is open, and close it when the owner closes
//! it.
//!
//! **Position:** drawn by the application's frame through `show_viewport_immediate`; where the
//! backend has one window only (the headless tests), egui draws it as a window inside the main
//! one.
//!
//! **Signals and state:** the place it opened at lives in egui's memory until it closes, so it
//! stays where the owner put it; asks for a frame every [`REFRESH`] while open, so new lines show.
//!
//! **Invariants:** it is drawn only while `log_window` is set; asking for it while it is open
//! brings it to the front.

use std::time::Duration;

use eframe::egui::{Context, Id, Pos2, ViewportBuilder, ViewportCommand, ViewportId, pos2};

use super::{Action, TbdSubtitlesApp, feature_views};

/// The window's size when it opens, and its smallest.
const SIZE: [f32; 2] = [980.0, 560.0];
const MIN_SIZE: [f32; 2] = [560.0, 280.0];
/// How often the open window looks for new lines.
const REFRESH: Duration = Duration::from_millis(250);

fn viewport() -> ViewportId {
    ViewportId::from_hash_of("log")
}

/// Draw the log window while it is open, in front when `raise`; push what it asks for onto
/// `actions`.
pub(super) fn log_window_ui(
    ctx: &Context,
    app: &TbdSubtitlesApp,
    raise: bool,
    actions: &mut Vec<Action>,
) {
    let place = Id::new("log-window-place");
    if !app.log_window {
        ctx.data_mut(|data| data.remove::<Pos2>(place));
        return;
    }
    ctx.request_repaint_after(REFRESH);
    let position = ctx.data(|data| data.get_temp::<Pos2>(place)).or_else(|| {
        let main = ctx.input(|input| input.viewport().outer_rect)?;
        let at = pos2(
            main.right() - SIZE[0] - 24.0,
            main.bottom() - SIZE[1] - 24.0,
        );
        ctx.data_mut(|data| data.insert_temp(place, at));
        Some(at)
    });
    let mut builder = ViewportBuilder::default()
        .with_title("Log")
        .with_inner_size(SIZE)
        .with_min_inner_size(MIN_SIZE);
    if let Some(at) = position {
        builder = builder.with_position(at);
    }
    if raise {
        ctx.send_viewport_cmd_to(viewport(), ViewportCommand::Focus);
    }
    ctx.show_viewport_immediate(viewport(), builder, |ui, _class| {
        if ui.ctx().input(|input| input.viewport().close_requested()) {
            actions.push(Action::ShowLog(false));
        }
        feature_views::log_console_ui(ui, app, actions);
    });
}
