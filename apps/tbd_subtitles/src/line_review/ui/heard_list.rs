//! What was heard: the language model's pick, Parakeet and Whisper, and when the line was heard
//! again on the voices alone, both engines' second listen; each with Use, or "In use" for the
//! reading the line's text is now.
//!
//! **Role:** draw the readings of the open line in one card and ask for the one the owner takes.
//!
//! **Position:** drawn by `line_editor` under "In the subtitles now".
//!
//! **Signals and state:** none.
//!
//! **Invariants:** a reading is in use when it matches the line's text as it stands now, ignoring
//! case and the space at its ends; an engine that heard nothing says so and offers nothing.

use eframe::egui::{
    Align, CornerRadius, Frame, Label, Layout, Margin, RichText, Sense, Stroke, Ui, vec2,
};

use crate::core::ui::button::{Button, ButtonSize};
use crate::core::ui::icons;
use crate::core::ui::palette::palette;
use crate::line_review::events::ReviewEvent;
use crate::line_review::models::session::{ReviewLine, ReviewSession};

/// The width of a reading's source, and room for its button.
const SOURCE_WIDTH: f32 = 150.0;
const ACTION_WIDTH: f32 = 64.0;

/// Draw the readings of `line`.
pub(super) fn heard_list_ui(
    ui: &mut Ui,
    session: &ReviewSession,
    line: &ReviewLine,
    events: &mut Vec<ReviewEvent>,
) {
    let p = palette(ui);
    let mut readings = vec![
        ("Language model's pick", "adjudicated"),
        ("Parakeet", "P"),
        ("Whisper", "W"),
    ];
    if line.reading("ALT p").is_some() || line.reading("ALT w").is_some() {
        readings.push(("Parakeet, voices only", "ALT p"));
        readings.push(("Whisper, voices only", "ALT w"));
    }
    let now = session.current(line).text.trim().to_lowercase();
    Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(CornerRadius::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            for (i, (source, tag)) in readings.iter().enumerate() {
                if i > 0 {
                    let (rule, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                    ui.painter().rect_filled(rule, 0.0, p.line);
                }
                let text = line.reading(tag).map(str::trim).filter(|t| !t.is_empty());
                let in_use = text.is_some_and(|t| t.to_lowercase() == now);
                row_ui(ui, source, text, in_use, || {
                    events.push(ReviewEvent::Pick((*tag).to_string()));
                });
            }
        });
}

/// One reading: its source, its text or "heard nothing", and Use or "In use".
fn row_ui(ui: &mut Ui, source: &str, text: Option<&str>, in_use: bool, pick: impl FnOnce()) {
    let p = palette(ui);
    Frame::new()
        .inner_margin(Margin {
            left: 12,
            right: 10,
            top: 7,
            bottom: 7,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                ui.allocate_ui_with_layout(
                    vec2(SOURCE_WIDTH, 20.0),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.set_width(SOURCE_WIDTH);
                        ui.label(RichText::new(source).size(12.0).color(p.text2));
                    },
                );
                let width = (ui.available_width() - ACTION_WIDTH - 10.0).max(40.0);
                ui.allocate_ui_with_layout(
                    vec2(width, 20.0),
                    Layout::left_to_right(Align::Center).with_main_wrap(true),
                    |ui| {
                        ui.set_width(width);
                        let words = match text {
                            Some(text) => RichText::new(text).color(p.text),
                            None => RichText::new("heard nothing").italics().color(p.text2),
                        };
                        ui.add(Label::new(words).wrap());
                    },
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if in_use {
                        in_use_ui(ui);
                    } else if text.is_some()
                        && Button::new("Use")
                            .size(ButtonSize::Small)
                            .show(ui)
                            .clicked()
                    {
                        pick();
                    }
                });
            });
        });
}

/// "In use" with a check, in green.
fn in_use_ui(ui: &mut Ui) {
    let p = palette(ui);
    ui.spacing_mut().item_spacing.x = 4.0;
    // Right to left: the words, then the check before them.
    ui.label(RichText::new("In use").size(11.5).color(p.good));
    ui.label(
        RichText::new(icons::CHECK)
            .font(icons::font(14.0))
            .color(p.good),
    );
}
