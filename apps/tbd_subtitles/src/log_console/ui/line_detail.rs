//! The detail panel under the activity list: the open line whole, wrapped, with where it came
//! from.
//!
//! **Role:** show the selected line's level, writer, source and time, then its video and step,
//! then its whole message wrapped and selectable, with Copy; a line that sums up a model call
//! offers Show Model Call.
//!
//! **Position:** drawn by `console_window` as a resizable bottom panel while a line is open.
//!
//! **Signals and state:** its height lives in egui's memory; otherwise none.
//!
//! **Invariants:** ✕ and Esc close it; the message is shown whole, however long.

use eframe::egui::{
    Align, Frame, Id, Key, Label, Layout, Margin, Panel, RichText, ScrollArea, TextStyle, Ui,
};
use tracing::Level;

use crate::core::log_buffer::LogLine;
use crate::core::ui::button::{Button, icon_button};
use crate::core::ui::icons;
use crate::core::ui::palette::palette;
use crate::log_console::events::LogConsoleEvent;
use crate::log_console::models::who::Who;
use crate::log_console::services::console_text;

/// Draw the panel for `line`, and push what the owner asked for onto `events`.
pub(crate) fn line_detail_ui(ui: &mut Ui, line: &LogLine, events: &mut Vec<LogConsoleEvent>) {
    let p = palette(ui);
    if ui.input(|input| input.key_pressed(Key::Escape)) {
        events.push(LogConsoleEvent::SelectLine(None));
    }
    Panel::bottom(Id::new("log-line-detail"))
        .resizable(true)
        .default_size(170.0)
        .min_size(90.0)
        .frame(
            Frame::new()
                .fill(p.card)
                .inner_margin(Margin::symmetric(12, 8)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let colour = match line.level {
                    Level::ERROR => p.bad,
                    Level::WARN => p.warn,
                    Level::INFO => p.accent,
                    _ => p.text3,
                };
                ui.label(RichText::new(line.level.as_str()).strong().color(colour));
                ui.label(
                    RichText::new(Who::of(&line.target).label())
                        .strong()
                        .color(p.text),
                );
                ui.label(RichText::new(&line.target).monospace().color(p.text2));
                ui.label(
                    RichText::new(console_text::time(line.elapsed).trim())
                        .monospace()
                        .color(p.text3),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if icon_button(ui, icons::X, "Close")
                        .on_hover_text("Close (Esc)")
                        .clicked()
                    {
                        events.push(LogConsoleEvent::SelectLine(None));
                    }
                    if Button::new("Copy").icon(icons::COPY).show(ui).clicked() {
                        ui.ctx().copy_text(line.message.clone());
                    }
                    if let Some(call) = &line.call
                        && Button::new("Show Model Call")
                            .icon(icons::MAGNIFYING_GLASS)
                            .show(ui)
                            .on_hover_text("What was sent to the model and what came back")
                            .clicked()
                    {
                        events.push(LogConsoleEvent::ShowCall(call.clone()));
                    }
                });
            });
            let place = console_text::header(line.video.as_deref(), line.step.as_deref());
            if !place.is_empty() {
                ui.label(RichText::new(place).color(p.text2));
            }
            ui.add_space(6.0);
            ScrollArea::vertical()
                .id_salt("log-line-detail-text")
                .auto_shrink(false)
                .show(ui, |ui| {
                    ui.add(
                        Label::new(
                            RichText::new(&line.message)
                                .text_style(TextStyle::Monospace)
                                .color(p.text),
                        )
                        .wrap()
                        .selectable(true),
                    );
                });
        });
}
