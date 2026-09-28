//! The Model Calls view: the calls on the left, and the open call on the right with what was sent
//! and what came back.
//!
//! **Role:** draw the borrowed `Calls` list (each call's purpose, then its time, place, model and
//! length) and the open call's header and four parts, each foldable and copyable; turn a click on
//! a call into `SelectCall`.
//!
//! **Position:** drawn by `console_window` while the Model Calls view shows.
//!
//! **Signals and state:** which parts are open lives in egui's memory (the answer opens first);
//! otherwise none.
//!
//! **Invariants:** only the list's rows in view are laid out; a part is shown whole, wrapped,
//! never cut; a failed call says why in red.

use eframe::egui::{
    Align, CentralPanel, CornerRadius, FontFamily, FontId, Frame, Id, Label, Layout, Margin, Panel,
    RichText, ScrollArea, Sense, TextStyle, Ui, WidgetInfo, WidgetType, pos2, vec2,
};

use super::activity_view::one_line;
use crate::core::log_buffer::KeptCall;
use crate::core::ui::button::Button;
use crate::core::ui::disclosure::disclosure;
use crate::core::ui::fonts;
use crate::core::ui::icons;
use crate::core::ui::palette::{Palette, palette};
use crate::log_console::events::LogConsoleEvent;
use crate::log_console::models::calls::Calls;
use crate::log_console::services::{call_text, console_text};

/// A call's row in the list, and the list's width when the window opens.
const ROW: f32 = 46.0;
const LIST_WIDTH: f32 = 360.0;

/// Draw the view, or why it is empty.
pub(crate) fn calls_ui(ui: &mut Ui, calls: &Calls, events: &mut Vec<LogConsoleEvent>) {
    let p = palette(ui);
    if calls.len() == 0 {
        CentralPanel::default()
            .frame(Frame::new().fill(p.window))
            .show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new(
                            "No model calls yet. They show here as a job settles its words or \
                             Fix It runs.",
                        )
                        .color(p.text3),
                    );
                });
            });
        return;
    }
    Panel::left(Id::new("log-calls-list"))
        .resizable(true)
        .default_size(LIST_WIDTH)
        .min_size(240.0)
        .frame(Frame::new().fill(p.sidebar).inner_margin(Margin::same(6)))
        .show(ui, |ui| list_ui(ui, p, calls, events));
    CentralPanel::default()
        .frame(
            Frame::new()
                .fill(p.window)
                .inner_margin(Margin::symmetric(16, 12)),
        )
        .show(ui, |ui| match calls.selected() {
            Some(kept) => call_ui(ui, p, kept),
            None => {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new(
                            "Choose a call on the left to see what was sent and what came back.",
                        )
                        .color(p.text3),
                    );
                });
            }
        });
}

/// The calls that pass the search, newest last.
fn list_ui(ui: &mut Ui, p: &Palette, calls: &Calls, events: &mut Vec<LogConsoleEvent>) {
    if calls.shown_len() == 0 {
        ui.label(RichText::new("No call matches the filter.").color(p.text3));
        return;
    }
    ui.spacing_mut().item_spacing.y = 0.0;
    let open = calls.selected().map(|kept| kept.call.id.clone());
    ScrollArea::vertical()
        .id_salt("log-calls")
        .auto_shrink(false)
        .stick_to_bottom(true)
        .show_rows(ui, ROW, calls.shown_len(), |ui, rows| {
            for row in rows {
                if let Some(kept) = calls.shown_call(row) {
                    let is_open = open.as_deref() == Some(kept.call.id.as_str());
                    if row_ui(ui, p, kept, is_open) && !is_open {
                        events.push(LogConsoleEvent::SelectCall(Some(kept.call.id.clone())));
                    }
                }
            }
        });
}

/// One call: its purpose (red when it failed), then its time, place, model and length.
fn row_ui(ui: &mut Ui, p: &Palette, kept: &KeptCall, open: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click());
    let call = &kept.call;
    let purpose = if call.purpose.is_empty() {
        "A call with no purpose given"
    } else {
        call.purpose.as_str()
    };
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, purpose));
    if open {
        ui.painter()
            .rect_filled(rect.shrink(1.0), CornerRadius::same(6), p.accent_tint);
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect.shrink(1.0), CornerRadius::same(6), p.hover);
    }
    let width = rect.width() - 16.0;
    let title_colour = if call.error.is_some() { p.bad } else { p.text };
    let semibold = FontId::new(12.5, FontFamily::Name(fonts::SEMIBOLD.into()));
    let mut title = purpose.to_string();
    if call.error.is_some() {
        title = format!("{} {title}", icons::WARNING);
    }
    let title = one_line(ui, &title, semibold, title_colour, width);
    ui.painter().galley(
        pos2(rect.left() + 8.0, rect.top() + 6.0),
        title,
        title_colour,
    );
    let under = format!(
        "{} · {} · {} · {:.1} s",
        console_text::time(kept.elapsed).trim(),
        call_text::place(kept),
        call.model,
        call.seconds
    );
    let under = one_line(ui, &under, FontId::proportional(11.5), p.text2, width);
    ui.painter()
        .galley(pos2(rect.left() + 8.0, rect.top() + 25.0), under, p.text2);
    response.clicked()
}

/// The open call: its header, then the answer, the message, the system prompt and the schema.
fn call_ui(ui: &mut Ui, p: &Palette, kept: &KeptCall) {
    let call = &kept.call;
    ui.label(
        RichText::new(if call.purpose.is_empty() {
            "A call with no purpose given"
        } else {
            &call.purpose
        })
        .size(15.0)
        .strong()
        .color(p.text),
    );
    ui.label(RichText::new(call_text::place(kept)).color(p.text2));
    ui.label(
        RichText::new(format!(
            "{} · call {} · {}",
            call.model,
            call.id,
            call_text::summary(kept)
        ))
        .color(p.text2),
    );
    if let Some(error) = &call.error {
        ui.label(RichText::new(format!("Failed: {error}")).color(p.bad));
    }
    ui.add_space(8.0);
    ScrollArea::vertical()
        .id_salt(("log-call", &call.id))
        .auto_shrink(false)
        .show(ui, |ui| {
            let answer_title = if call.error.is_some() {
                "What claude printed"
            } else {
                "Answer"
            };
            part_ui(ui, p, &call.id, answer_title, &call.answer, true);
            part_ui(ui, p, &call.id, "Message", &call.message, false);
            part_ui(ui, p, &call.id, "System prompt", &call.system, false);
            part_ui(ui, p, &call.id, "Schema", &call.schema, false);
        });
}

/// One part of a call under a disclosure, with its size and Copy; open at first when `open`.
fn part_ui(ui: &mut Ui, p: &Palette, id: &str, title: &str, text: &str, open: bool) {
    let key = Id::new(("log-call-part", id, title));
    if open {
        ui.data_mut(|data| {
            data.get_temp_mut_or_insert_with(key, || true);
        });
    }
    let size = format!(
        "{} lines · {} characters",
        text.lines().count(),
        text.chars().count()
    );
    if !disclosure(ui, key, (title, title), &size) {
        return;
    }
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if Button::new("Copy").icon(icons::COPY).show(ui).clicked() {
                ui.ctx().copy_text(text.to_string());
            }
        });
    });
    Frame::new()
        .fill(p.well)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let shown = if text.is_empty() { "(empty)" } else { text };
            ui.add(
                Label::new(
                    RichText::new(shown)
                        .text_style(TextStyle::Monospace)
                        .color(p.text),
                )
                .wrap()
                .selectable(true),
            );
        });
    ui.add_space(8.0);
}
