//! The line editor on the right of Check Lines: the head and the why boxes (`line_status`), the
//! line's clip, the text in the subtitles now, what was heard, the text box with its `||` hint,
//! the four flags, and the footer; or what to do when no line is shown.||` hint, the four flags, and the footer; or what to do when
//! no line is shown.
//!
//! **Role:** draw the open line from the borrowed view and turn every click and edit into a
//! `ReviewEvent`.
//!
//! **Position:** drawn by `review_view` in its central panel; draws `line_status`, `clip_view` and
//! `heard_list`.
//!
//! **Signals and state:** the text box's cursor lives in egui's memory under the line's id.
//!
//! **Invariants:** the footer offers Discard Edit and Save Correction while the line is edited,
//! Keep Change and Undo Change while Fix It's change waits for the owner, Take Back while the
//! owner corrected it, and Looks Right otherwise; Previous and Next are off at the
//! list's ends; nothing is saved while a full run of the video runs; each flag's caption says
//! what the subtitles do with it.

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    Align, CornerRadius, EventFilter, FontFamily, FontId, Frame, Id, Label, Layout, Margin, Panel,
    Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui, UiBuilder, pos2, vec2,
};

use super::clip_view::clip_view_ui;
use super::heard_list::heard_list_ui;
use super::line_status::{head_ui, why_ui};
use super::review_view::{ReviewView, small_caps};
use crate::core::format;
use crate::core::ui::button::Button;
use crate::core::ui::fonts;
use crate::core::ui::icons::{self, StatusIcon, status_icon};
use crate::core::ui::palette::palette;
use crate::core::ui::switch::switch;
use crate::line_review::events::ReviewEvent;
use crate::line_review::models::session::{LineList, ReviewLine, ReviewSession};
use crate::line_review::services::line_filter;
use crate::line_review::services::review_editing::{self, EDITABLE_FLAGS};

/// Why nothing can be saved now.
const BUSY: &str = "The video is being processed again; save once it ends.";

/// Draw the editor of `line`: the footer pinned at the bottom, the rest scrolling above it.
pub(super) fn line_editor_ui(
    ui: &mut Ui,
    view: &ReviewView<'_>,
    line: &ReviewLine,
    events: &mut Vec<ReviewEvent>,
) {
    let p = palette(ui);
    Panel::bottom(Id::new("review-footer"))
        .frame(
            Frame::new()
                .fill(p.window)
                .inner_margin(Margin::symmetric(22, 10)),
        )
        .show(ui, |ui| footer_ui(ui, view, line, events));
    ScrollArea::vertical()
        .id_salt(("review-editor", line.id.as_str()))
        .auto_shrink(false)
        .show(ui, |ui| {
            Frame::new()
                .inner_margin(Margin {
                    left: 22,
                    right: 22,
                    top: 16,
                    bottom: 20,
                })
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 14.0;
                    head_ui(ui, view.session, line);
                    why_ui(ui, view.session, line);
                    clip_view_ui(ui, view, line, events);
                    now_ui(ui, view.session, line);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        small_caps(ui, "What was heard");
                        heard_list_ui(ui, view.session, line, events);
                    });
                    text_ui(ui, view.session, line, events);
                    flags_ui(ui, view.session, line, events);
                });
        });
}

/// The line's text as the subtitle file has it now.
fn now_ui(ui: &mut Ui, session: &ReviewSession, line: &ReviewLine) {
    let p = palette(ui);
    let text = session
        .correction(&line.id)
        .map_or(line.adjudicated.as_str(), |c| c.text.as_str());
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        small_caps(ui, "In the subtitles now");
        Frame::new()
            .fill(p.card)
            .stroke(Stroke::new(1.0, p.line))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::symmetric(12, 9))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(Label::new(RichText::new(text).size(15.0).color(p.text)).wrap());
            });
    });
}

/// "Your text", the text box, and the hint that `||` starts a second speaker.
fn text_ui(ui: &mut Ui, session: &ReviewSession, line: &ReviewLine, events: &mut Vec<ReviewEvent>) {
    let p = palette(ui);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 5.0;
        small_caps(ui, "Your text");
        let now = session.current(line).text;
        let mut text = now.clone();
        let edit = TextEdit::multiline(&mut text)
            .id(Id::new(("review-line-text", line.id.as_str())))
            .desired_rows(2)
            .desired_width(f32::INFINITY)
            .font(FontId::proportional(14.0))
            .margin(Margin::symmetric(10, 8))
            .event_filter(EventFilter {
                horizontal_arrows: true,
                vertical_arrows: true,
                escape: true,
                ..EventFilter::default()
            });
        // The box shows the line as it stands each frame; only a change the owner made is sent.
        if ui.add(edit).changed() && text != now {
            events.push(ReviewEvent::EditText(text));
        }
        let hint = TextFormat::simple(FontId::proportional(11.5), p.text2);
        let mut job = LayoutJob::default();
        job.append("Type ", 0.0, hint.clone());
        job.append(
            "||",
            0.0,
            TextFormat {
                font_id: FontId::monospace(11.5),
                color: p.text2,
                background: p.well,
                ..TextFormat::default()
            },
        );
        job.append(" where a second speaker starts.", 0.0, hint.clone());
        job.append("Esc leaves the text box.", 12.0, hint);
        ui.add(Label::new(job).wrap());
    });
}

/// The four flags, two to a row, each a card with its switch, name and what it does.
fn flags_ui(
    ui: &mut Ui,
    session: &ReviewSession,
    line: &ReviewLine,
    events: &mut Vec<ReviewEvent>,
) {
    let p = palette(ui);
    let flags = session.current(line).flags;
    let column = ((ui.available_width() - 16.0) / 2.0).max(120.0);
    let caption_width = column - 20.0 - 2.0 - 32.0 - 10.0;
    ui.spacing_mut().item_spacing.y = 8.0;
    for pair in EDITABLE_FLAGS.chunks(2) {
        let laid: Vec<_> = pair
            .iter()
            .map(|flag| {
                let (name, caption) = flag_words(flag);
                let bold = FontId::new(13.0, FontFamily::Name(fonts::BOLD.into()));
                let name = ui.painter().layout_no_wrap(name.to_string(), bold, p.text);
                let caption = ui.painter().layout(
                    caption.to_string(),
                    FontId::proportional(11.5),
                    p.text2,
                    caption_width,
                );
                (*flag, name, caption)
            })
            .collect();
        let height = laid
            .iter()
            .map(|(_, name, caption)| 16.0 + name.size().y + 1.0 + caption.size().y)
            .fold(0.0, f32::max);
        let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
        for (i, (flag, name, caption)) in laid.into_iter().enumerate() {
            let left = row.left() + i as f32 * (column + 16.0);
            let card = Rect::from_min_size(pos2(left, row.top()), vec2(column, height));
            ui.painter().rect(
                card,
                CornerRadius::same(8),
                p.card,
                Stroke::new(1.0, p.line),
                StrokeKind::Inside,
            );
            let on = flags.iter().any(|f| f == flag);
            let knob = Rect::from_min_size(card.min + vec2(11.0, 9.0), vec2(32.0, 19.0));
            let (label, _) = flag_words(flag);
            // A child that takes no room: the row already holds the card.
            let mut place = ui.new_child(UiBuilder::new().max_rect(knob));
            if switch(&mut place, on, label).clicked() {
                let mut changed: Vec<String> = flags
                    .iter()
                    .filter(|f| f.as_str() != flag)
                    .cloned()
                    .collect();
                if !on {
                    changed.push(flag.to_string());
                }
                events.push(ReviewEvent::SetFlags(changed));
            }
            let text_left = knob.right() + 10.0;
            let name_height = name.size().y;
            ui.painter()
                .galley(pos2(text_left, card.top() + 8.0), name, p.text);
            ui.painter().galley(
                pos2(text_left, card.top() + 8.0 + name_height + 1.0),
                caption,
                p.text2,
            );
        }
    }
}

/// A flag's name and what the subtitles do with it.
fn flag_words(flag: &str) -> (&'static str, &'static str) {
    match flag {
        "SPK" => (
            "New speaker",
            "Never merged into the line before; they share a subtitle only with dashes",
        ),
        "NARR" => (
            "Narrator",
            "In italics, and never in a two-speaker subtitle",
        ),
        "LYRIC" => (
            "Song lyric",
            "Left out of the dialogue; its song can get a sound cue",
        ),
        _ => ("Drop the line", "Left out of the subtitles"),
    }
}

/// Previous and Next; then Discard Edit and Save Correction, Keep Change and Undo Change, Take
/// Back, or Looks Right.
fn footer_ui(ui: &mut Ui, view: &ReviewView<'_>, line: &ReviewLine, events: &mut Vec<ReviewEvent>) {
    let session = view.session;
    let p = palette(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let previous = Button::new("Previous")
            .icon(icons::CARET_LEFT)
            .enabled(line_filter::neighbour(session, false).is_some())
            .show(ui)
            .on_hover_text("Previous line (↑)");
        if previous.clicked() {
            events.push(ReviewEvent::Step { forward: false });
        }
        let next = Button::new("Next")
            .icon_after(icons::CARET_RIGHT)
            .enabled(line_filter::neighbour(session, true).is_some())
            .show(ui)
            .on_hover_text("Next line (↓)");
        if next.clicked() {
            events.push(ReviewEvent::Step { forward: true });
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let free = !view.job_busy;
            if review_editing::is_dirty(session, &line.id) {
                let save = Button::new("Save Correction")
                    .primary(true)
                    .hint("Ctrl+S")
                    .enabled(free)
                    .show(ui)
                    .on_disabled_hover_text(BUSY);
                if save.clicked() {
                    events.push(ReviewEvent::Save);
                }
                if Button::new("Discard Edit").show(ui).clicked() {
                    events.push(ReviewEvent::Discard);
                }
                ui.label(RichText::new("Edited, not saved").size(11.5).color(p.text2));
            } else if session.unchecked_fix(&line.id) {
                let keep = Button::new("Keep Change")
                    .icon(icons::CHECK)
                    .primary(true)
                    .hint("Ctrl+Enter")
                    .enabled(free)
                    .show(ui)
                    .on_hover_text("Keep Claude's text. It becomes yours.")
                    .on_disabled_hover_text(BUSY);
                if keep.clicked() {
                    events.push(ReviewEvent::LooksRight);
                }
                let undo = Button::new("Undo Change")
                    .icon(icons::ARROW_COUNTER_CLOCKWISE)
                    .enabled(free)
                    .show(ui)
                    .on_hover_text("Put the app's reading back. The line is timed again.")
                    .on_disabled_hover_text(BUSY);
                if undo.clicked() {
                    events.push(ReviewEvent::UndoChange);
                }
            } else if session.correction(&line.id).is_some() {
                let back = Button::new("Take Back")
                    .icon(icons::ARROW_COUNTER_CLOCKWISE)
                    .enabled(free)
                    .show(ui)
                    .on_disabled_hover_text(BUSY);
                if back.clicked() {
                    events.push(ReviewEvent::Revert(line.id.clone()));
                }
            } else {
                let keep = Button::new("Looks Right")
                    .icon(icons::CHECK)
                    .primary(true)
                    .hint("Ctrl+Enter")
                    .enabled(free)
                    .show(ui)
                    .on_hover_text(
                        "Keep this text. The line is timed again and its warnings clear.",
                    )
                    .on_disabled_hover_text(BUSY);
                if keep.clicked() {
                    events.push(ReviewEvent::LooksRight);
                }
            }
        });
    });
}

/// What the editor shows when the list shows no line: every line checked, or a hint.
pub(super) fn nothing_open_ui(ui: &mut Ui, session: &ReviewSession, events: &mut Vec<ReviewEvent>) {
    let p = palette(ui);
    let done = session.list == LineList::ToCheck
        && session.search.trim().is_empty()
        && session.group.is_none();
    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.add_space((ui.available_height() / 2.0 - 70.0).max(24.0));
        ui.spacing_mut().item_spacing.y = 8.0;
        if done {
            let flagged = line_filter::worth(session);
            status_icon(ui, StatusIcon::Done, 40.0, false);
            let bold = FontId::new(17.0, FontFamily::Name(fonts::BOLD.into()));
            ui.label(
                RichText::new(format!("All {} checked", format::plural(flagged, "line")))
                    .font(bold)
                    .color(p.text),
            );
            ui.label(RichText::new("The subtitles are up to date.").color(p.text2));
            if Button::new("Show Checked Lines").show(ui).clicked() {
                events.push(ReviewEvent::List(LineList::Checked));
            }
        } else {
            ui.label(
                RichText::new(icons::LIST)
                    .font(icons::font(40.0))
                    .color(p.text3),
            );
            ui.label(RichText::new("Choose a filter on the left.").color(p.text2));
        }
    });
}
