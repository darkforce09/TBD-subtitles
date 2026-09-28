//! The Settings window: a second native window, centred over the main window when it opens,
//! that shows the settings page.
//!
//! **Role:** show the settings in their own window while it is open, and close it when the owner
//! closes it.
//!
//! **Position:** drawn by the application's frame through `show_viewport_immediate`; where the
//! backend has one window only (the headless tests), egui draws it as a window inside the main
//! one.
//!
//! **Signals and state:** the place it opened at lives in egui's memory until it closes, so it
//! stays where the owner put it.
//!
//! **Invariants:** it is drawn only while `settings_window` is set; asking for it while it is
//! open brings it to the front.

use eframe::egui::{
    CentralPanel, Context, Id, Pos2, ViewportBuilder, ViewportCommand, ViewportId, vec2,
};

use super::{Action, TbdSubtitlesApp, feature_views};

/// The window's size when it opens, and its smallest.
const SIZE: [f32; 2] = [660.0, 600.0];
const MIN_SIZE: [f32; 2] = [480.0, 360.0];

fn viewport() -> ViewportId {
    ViewportId::from_hash_of("settings")
}

/// Draw the Settings window while it is open, in front when `raise`; push what it asks for onto
/// `actions`.
pub(super) fn settings_window_ui(
    ctx: &Context,
    app: &TbdSubtitlesApp,
    raise: bool,
    actions: &mut Vec<Action>,
) {
    let place = Id::new("settings-window-place");
    if !app.settings_window {
        ctx.data_mut(|data| data.remove::<Pos2>(place));
        return;
    }
    let position = ctx.data(|data| data.get_temp::<Pos2>(place)).or_else(|| {
        let main = ctx.input(|input| input.viewport().outer_rect)?;
        let at = main.center() - vec2(SIZE[0], SIZE[1]) / 2.0;
        ctx.data_mut(|data| data.insert_temp(place, at));
        Some(at)
    });
    let mut builder = ViewportBuilder::default()
        .with_title("Settings")
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
            actions.push(Action::ShowSettings(false));
        }
        CentralPanel::default().show(ui, |ui| feature_views::settings_ui(ui, app, actions));
    });
}
