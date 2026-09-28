//! The toolbar across the top of the window: Add Videos… and Add Folder…, the queue's one
//! button with the reason it is off, the log button that opens the log window, and the gear that
//! opens Settings.
//!
//! **Role:** draw the toolbar from the borrowed queue and turn each click into a
//! `JobQueueEvent`, or report the log button or the gear.
//!
//! **Position:** called by the application's frame for the top panel; the button comes from
//! `queue_editing::queue_control`.
//!
//! **Signals and state:** none; reads the view and returns events.
//!
//! **Invariants:** exactly one of Start Queue, Pause After This Video and Resume Queue shows; a
//! disabled Start Queue says why beside it.

use eframe::egui::{Align, Layout, RichText, Ui};

use crate::core::ui::button::{Button, ButtonSize, icon_button};
use crate::core::ui::icons;
use crate::core::ui::palette::palette;
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::view::JobQueueView;
use crate::job_queue::services::queue_editing::{QueueControl, queue_control};

/// Which of the toolbar's window buttons was pressed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ToolbarPress {
    /// The gear: open Settings.
    pub(crate) settings: bool,
    /// The log button: open the log window.
    pub(crate) log: bool,
}

/// Draw the toolbar and push what the owner asked for onto `events`; which window button was
/// pressed.
pub(crate) fn toolbar_ui(
    ui: &mut Ui,
    view: &JobQueueView<'_>,
    events: &mut Vec<JobQueueEvent>,
) -> ToolbarPress {
    let mut pressed = ToolbarPress::default();
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if Button::new("Add Videos…")
            .icon(icons::PLUS)
            .show(ui)
            .on_hover_text("Add videos (Ctrl+O)")
            .clicked()
        {
            events.push(JobQueueEvent::AddVideos);
        }
        if Button::new("Add Folder…")
            .icon(icons::FOLDER_PLUS)
            .show(ui)
            .on_hover_text("Add every video in a folder (Ctrl+Shift+O)")
            .clicked()
        {
            events.push(JobQueueEvent::AddFolder);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if icon_button(ui, icons::GEAR, "Settings")
                .on_hover_text("Settings (Ctrl+,)")
                .clicked()
            {
                pressed.settings = true;
            }
            if icon_button(ui, icons::TERMINAL, "Log")
                .on_hover_text("Log (Ctrl+L)")
                .clicked()
            {
                pressed.log = true;
            }
            let (label, glyph, primary, enabled, reason, event) =
                match queue_control(view.queue, view.models_missing) {
                    QueueControl::Start { reason } => (
                        "Start Queue",
                        icons::PLAY,
                        true,
                        reason.is_none(),
                        reason.map(String::from),
                        JobQueueEvent::Start,
                    ),
                    QueueControl::PauseAfter => (
                        "Pause After This Video",
                        icons::PAUSE,
                        false,
                        true,
                        None,
                        JobQueueEvent::Pause,
                    ),
                    QueueControl::Resume { reason } => (
                        "Resume Queue",
                        icons::PLAY,
                        true,
                        true,
                        Some(reason),
                        JobQueueEvent::Start,
                    ),
                };
            let mut button = Button::new(label)
                .icon(glyph)
                .primary(primary)
                .size(ButtonSize::Large)
                .enabled(enabled)
                .min_width(124.0)
                .show(ui);
            if let Some(reason) = reason.as_ref().filter(|_| !enabled) {
                button = button.on_hover_text(reason);
            }
            if button.clicked() {
                events.push(event);
            }
            if let Some(reason) = reason {
                ui.add_space(6.0);
                ui.spacing_mut().item_spacing.x = 6.0;
                let text2 = palette(ui).text2;
                ui.label(RichText::new(reason).size(12.0).color(text2));
                if !enabled {
                    ui.label(
                        RichText::new(icons::INFO)
                            .font(icons::font(14.0))
                            .color(text2),
                    );
                }
            }
        });
    });
    pressed
}
