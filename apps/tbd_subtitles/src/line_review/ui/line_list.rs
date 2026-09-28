//! The list of lines on the left of Check Lines: To Check, Checked and All with their counts, the
//! search field, the group the list is narrowed to, and a row per line with its mark, time, text
//! and chips.
//!
//! **Role:** draw the list's tools and rows from the session, and turn a click or a typed search
//! into a `ReviewEvent`.
//!
//! **Position:** drawn by `review_view` in its left panel; reads `line_filter` and
//! `review_editing::status`.
//!
//! **Signals and state:** remembers in egui's memory which line was open last, to scroll a newly
//! opened line into view.
//!
//! **Invariants:** a row shows a line's text as it stands now, draft included, cut after two
//! lines; an edited row says so before anything else, a checked row says how, and a row to check
//! names its groups.

use std::sync::Arc;

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    Align2, Color32, CornerRadius, EventFilter, FontFamily, FontId, Frame, Galley, Id, Margin,
    Rect, Response, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui, WidgetInfo,
    WidgetType, pos2, vec2,
};

use super::review_view::clickable;
use crate::core::format;
use crate::core::ui::fonts;
use crate::core::ui::icons;
use crate::core::ui::palette::palette;
use crate::core::ui::segmented::{Tally, segmented_across};
use crate::job_report::models::finding_group::LineGroup;
use crate::line_review::events::ReviewEvent;
use crate::line_review::models::session::{LineList, LineStatus, ReviewLine, ReviewSession};
use crate::line_review::services::{line_filter, review_editing};

/// A row's padding: above and below, on the left and on the right; its mark's column and the gap
/// after it.
const ROW_Y: f32 = 9.0;
const ROW_LEFT: f32 = 10.0;
const ROW_RIGHT: f32 = 12.0;
const MARK: f32 = 14.0;
const MARK_GAP: f32 = 8.0;
/// A chip's height, its side padding, and the gap between chips.
const CHIP_HEIGHT: f32 = 18.0;
const CHIP_PADDING: f32 = 6.0;
const CHIP_GAP: f32 = 4.0;

/// Draw the tools and the rows.
pub(super) fn line_list_ui(ui: &mut Ui, session: &ReviewSession, events: &mut Vec<ReviewEvent>) {
    let p = palette(ui);
    ui.spacing_mut().item_spacing.y = 0.0;
    Frame::new()
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 8.0;
            tools_ui(ui, session, events);
        });
    let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(line, 0.0, p.line);
    ScrollArea::vertical()
        .id_salt("review-list")
        .auto_shrink(false)
        .show(ui, |ui| rows_ui(ui, session, events));
}

/// To Check, Checked and All; the search field; the group the list is narrowed to.
fn tools_ui(ui: &mut Ui, session: &ReviewSession, events: &mut Vec<ReviewEvent>) {
    let counts = line_filter::counts(session);
    let lists = [
        (
            LineList::ToCheck,
            "To Check",
            Some(Tally::Count(counts.to_check)),
        ),
        (
            LineList::Checked,
            "Checked",
            Some(Tally::Count(counts.checked)),
        ),
        (LineList::All, "All", Some(Tally::Count(counts.all))),
    ];
    if let Some(list) = segmented_across(ui, session.list, &lists) {
        events.push(ReviewEvent::List(list));
    }
    search_ui(ui, &session.search, events);
    if let Some(group) = session.group {
        group_ui(ui, group, events);
    }
}

/// The search field: a grey well with a magnifier, white with a blue ring while typing.
fn search_ui(ui: &mut Ui, search: &str, events: &mut Vec<ReviewEvent>) {
    let p = palette(ui);
    let id = Id::new("review-search");
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::hover());
    let focused = ui.memory(|memory| memory.has_focus(id));
    let (fill, stroke) = if focused {
        (p.control, Stroke::new(2.0, p.accent))
    } else {
        (p.seg_bg, Stroke::NONE)
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(7),
        fill,
        stroke,
        StrokeKind::Outside,
    );
    ui.painter().text(
        pos2(rect.left() + 15.0, rect.center().y),
        Align2::CENTER_CENTER,
        icons::MAGNIFYING_GLASS,
        icons::font(14.0),
        p.text3,
    );
    let field = Rect::from_min_max(pos2(rect.left() + 28.0, rect.top()), rect.max);
    let mut text = search.to_string();
    let edit = TextEdit::singleline(&mut text)
        .id(id)
        .frame(Frame::new().inner_margin(Margin::symmetric(0, 6)))
        .hint_text("Search text or time")
        .desired_width(field.width() - 8.0)
        .event_filter(EventFilter {
            escape: true,
            ..EventFilter::default()
        });
    if ui.put(field, edit).changed() {
        events.push(ReviewEvent::Search(text));
    }
}

/// "Showing" and the group's orange pill, whose ✕ shows every group again.
fn group_ui(ui: &mut Ui, group: LineGroup, events: &mut Vec<ReviewEvent>) {
    let p = palette(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(RichText::new("Showing").size(12.0).color(p.text2));
        let semibold = FontId::new(11.5, FontFamily::Name(fonts::SEMIBOLD.into()));
        let title = ui
            .painter()
            .layout_no_wrap(group.title().to_string(), semibold, p.warn);
        let cross = ui
            .painter()
            .layout_no_wrap(icons::X.to_string(), icons::font(14.0), p.warn);
        let width = 8.0 + title.size().x + 6.0 + cross.size().x + 8.0;
        let (rect, _) = ui.allocate_exact_size(vec2(width, 20.0), Sense::hover());
        ui.painter()
            .rect_filled(rect, CornerRadius::same(10), p.warn_tint);
        let centre = rect.center().y;
        let title_width = title.size().x;
        ui.painter().galley(
            pos2(rect.left() + 8.0, centre - title.size().y / 2.0),
            title,
            p.warn,
        );
        let cross_rect = Rect::from_center_size(
            pos2(
                rect.left() + 8.0 + title_width + 6.0 + cross.size().x / 2.0,
                centre,
            ),
            cross.size(),
        );
        ui.painter().galley(cross_rect.min, cross, p.warn);
        let hit = cross_rect.expand(3.0);
        if clickable(ui, hit, Id::new("review-clear-group"), "Show every group").clicked() {
            events.push(ReviewEvent::ClearGroup);
        }
    });
}

/// A row per line shown, or why there is none.
fn rows_ui(ui: &mut Ui, session: &ReviewSession, events: &mut Vec<ReviewEvent>) {
    let shown = line_filter::shown(session);
    let open = line_filter::open_line(session).map(|line| line.id.clone());
    let last = Id::new("review-list-last-open");
    let scroll = ui.data(|d| d.get_temp::<Option<String>>(last)) != Some(open.clone());
    ui.data_mut(|d| d.insert_temp(last, open.clone()));
    for line in &shown {
        let selected = open.as_deref() == Some(line.id.as_str());
        let response = row_ui(ui, session, line, selected);
        if selected && scroll {
            response.scroll_to_me(None);
        }
        if response.clicked() {
            events.push(ReviewEvent::Open(line.id.clone()));
        }
    }
    if shown.is_empty() {
        let message = if !session.search.trim().is_empty() {
            format!("No lines match “{}”.", session.search.trim())
        } else if session.list == LineList::ToCheck {
            let group = if session.group.is_some() {
                " in this group"
            } else {
                ""
            };
            format!("Nothing left to check{group}.")
        } else {
            "No checked lines yet. Lines you keep or correct show here.".to_string()
        };
        let p = palette(ui);
        Frame::new().inner_margin(Margin::same(12)).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(message).size(12.0).color(p.text2));
            });
        });
    }
}

/// One line's row: its mark, time, text cut after two lines, and chips.
fn row_ui(ui: &mut Ui, session: &ReviewSession, line: &ReviewLine, selected: bool) -> Response {
    let p = palette(ui);
    let width = ui.available_width();
    let text_left = ROW_LEFT + MARK + MARK_GAP;
    let text_width = (width - text_left - ROW_RIGHT).max(40.0);
    let time = ui.painter().layout_no_wrap(
        format::clock_tenths(line.start_s),
        FontId::proportional(11.5),
        p.text2,
    );
    let words = session.current(line).text;
    let mut job = LayoutJob::simple(
        words.clone(),
        FontId::proportional(13.0),
        p.text,
        text_width,
    );
    job.wrap = TextWrapping {
        max_width: text_width,
        max_rows: 2,
        break_anywhere: false,
        overflow_character: Some('…'),
    };
    let text = ui.painter().layout_job(job);
    let status = review_editing::status(session, line);
    let chips = chips(ui, line, status);
    let chip_rows = chip_rows(&chips, text_width);
    let chips_height = if chips.is_empty() {
        0.0
    } else {
        3.0 + chip_rows as f32 * (CHIP_HEIGHT + CHIP_GAP) - CHIP_GAP
    };
    let height = ROW_Y + time.size().y + 3.0 + text.size().y + chips_height + ROW_Y;
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let name = format!("{} {words}", format::clock_tenths(line.start_s));
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, selected, &name));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, 0.0, p.accent_tint);
        let bar = Rect::from_min_max(rect.min, pos2(rect.left() + 3.0, rect.bottom()));
        painter.rect_filled(bar, 0.0, p.accent_fill);
    } else if response.hovered() {
        painter.rect_filled(rect, 0.0, p.hover);
    }
    let border = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 1.0), rect.max);
    painter.rect_filled(border, 0.0, p.line);
    let mark = pos2(
        rect.left() + ROW_LEFT + MARK / 2.0,
        rect.top() + ROW_Y + 7.0,
    );
    match status {
        LineStatus::Edited => {
            painter.circle_filled(mark, 3.5, p.accent_fill);
        }
        LineStatus::Kept | LineStatus::Corrected => {
            painter.text(
                mark,
                Align2::CENTER_CENTER,
                icons::CHECK,
                icons::font(14.0),
                p.good_icon,
            );
        }
        LineStatus::ToCheck => {
            painter.circle_filled(mark, 3.5, p.warn_icon);
        }
        LineStatus::Plain => {}
    }
    let mut y = rect.top() + ROW_Y;
    let left = rect.left() + text_left;
    let time_height = time.size().y;
    painter.galley(pos2(left, y), time, p.text2);
    y += time_height + 3.0;
    let text_height = text.size().y;
    painter.galley(pos2(left, y), text, p.text);
    y += text_height + 3.0;
    let mut x = left;
    for (galley, fill, colour) in chips {
        let chip_width = galley.size().x + 2.0 * CHIP_PADDING;
        if x > left && x + chip_width > left + text_width {
            x = left;
            y += CHIP_HEIGHT + CHIP_GAP;
        }
        let chip = Rect::from_min_size(pos2(x, y), vec2(chip_width, CHIP_HEIGHT));
        painter.rect_filled(chip, CornerRadius::same(5), fill);
        let at = chip.center() - galley.size() / 2.0;
        painter.galley(at, galley, colour);
        x += chip_width + CHIP_GAP;
    }
    response
}

/// A row's chips, laid out with their fill and text colour: "Edited, not saved" for an edited
/// line, "Looks right" or "Corrected" for a checked one, else one per group.
fn chips(ui: &Ui, line: &ReviewLine, status: LineStatus) -> Vec<(Arc<Galley>, Color32, Color32)> {
    let p = palette(ui);
    let words: Vec<(&str, Color32, Color32)> = match status {
        LineStatus::Edited => vec![("Edited, not saved", p.accent_tint, p.accent)],
        LineStatus::Kept => vec![("Looks right", p.good_tint, p.good)],
        LineStatus::Corrected => vec![("Corrected", p.good_tint, p.good)],
        LineStatus::ToCheck | LineStatus::Plain => line
            .groups
            .iter()
            .map(|(group, _)| (group.chip(), p.warn_tint, p.warn))
            .collect(),
    };
    words
        .into_iter()
        .map(|(word, fill, colour)| {
            let galley =
                ui.painter()
                    .layout_no_wrap(word.to_string(), FontId::proportional(11.0), colour);
            (galley, fill, colour)
        })
        .collect()
}

/// How many rows `chips` take in `width`.
fn chip_rows(chips: &[(Arc<Galley>, Color32, Color32)], width: f32) -> usize {
    let (mut rows, mut x) = (usize::from(!chips.is_empty()), 0.0);
    for (galley, _, _) in chips {
        let chip = galley.size().x + 2.0 * CHIP_PADDING;
        if x > 0.0 && x + chip > width {
            rows += 1;
            x = 0.0;
        }
        x += chip + CHIP_GAP;
    }
    rows
}
