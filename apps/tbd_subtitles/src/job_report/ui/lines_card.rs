//! The Overview's lines card: how many lines are worth a listen, a green bar of those checked,
//! Check Lines, and a row per group that opens Check Lines on it.
//!
//! **Role:** draw the borrowed report's line counts and turn the button and the rows into
//! `ReportEvent::CheckLines`.
//!
//! **Position:** the second card of `overview`; its head is `overview::head_ui`.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** a job with no line worth a listen shows a card saying so and no rows; a group
//! with no line has no row; a row is named by its group's title for accessibility.

use std::sync::Arc;

use eframe::egui::{
    Align, Align2, Color32, CornerRadius, FontFamily, FontId, Frame, Galley, Layout, Margin,
    RichText, Sense, Stroke, Ui, WidgetInfo, WidgetType, pos2, text::LayoutJob, vec2,
};

use crate::core::format;
use crate::core::ui::button::{Button, ButtonSize};
use crate::core::ui::card::{card, card_head};
use crate::core::ui::fonts;
use crate::core::ui::icons::{self, StatusIcon};
use crate::core::ui::palette::palette;
use crate::core::ui::progress::good_bar;
use crate::job_report::events::{LinesToCheck, ReportEvent};
use crate::job_report::models::finding_group::LineGroup;
use crate::job_report::models::report::JobReport;
use crate::job_report::ui::overview::head_ui;

/// The widest the bar of checked lines grows.
const BAR_WIDTH: f32 = 320.0;
/// A group row's space inside, between its parts, and its mark's size.
const ROW_SIDE: f32 = 18.0;
const ROW_TOP: f32 = 10.0;
const ROW_GAP: f32 = 12.0;
const ROW_ICON: f32 = 18.0;

/// Draw the lines card of `report`.
pub(super) fn lines_card_ui(ui: &mut Ui, report: &JobReport, events: &mut Vec<ReportEvent>) {
    let lines = &report.lines;
    if lines.flagged == 0 {
        card(ui, true, |ui| {
            card_head(ui, StatusIcon::Done, "No lines need a listen", |ui| {
                ui.label(
                    RichText::new("Every line was clear to both speech engines.")
                        .color(palette(ui).text2),
                );
            });
        });
        return;
    }
    let p = palette(ui);
    let (n, done) = (lines.flagged, lines.checked.min(lines.flagged));
    let left = n - done;
    card(ui, false, |ui| {
        Frame::new()
            .inner_margin(Margin::symmetric(18, 16))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 12.0;
                let (title, line) = if left > 0 {
                    (
                        format!("{} worth a listen", format::plural(n, "line")),
                        "Nothing is required. Each of these already has the app's best reading \
                         in the file. Listen, then keep it or correct it.",
                    )
                } else {
                    (
                        format!("All {} checked", format::plural(n, "line")),
                        "The subtitles are up to date with every check you made.",
                    )
                };
                head_ui(ui, (icons::EAR, p.accent), &title, line, |_| {});
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    let checked = format!("{done} of {n} checked");
                    let checked =
                        ui.painter()
                            .layout_no_wrap(checked, FontId::proportional(12.0), p.text2);
                    let room = ui.available_width() - checked.size().x - 180.0;
                    good_bar(ui, done as f32 / n as f32, room.clamp(40.0, BAR_WIDTH));
                    let (rect, _) = ui.allocate_exact_size(checked.size(), Sense::hover());
                    ui.painter().galley(rect.min, checked, p.text2);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let label = if left > 0 {
                            "Check Lines"
                        } else {
                            "Show Checked Lines"
                        };
                        let open = Button::new(label)
                            .icon(icons::EAR)
                            .primary(true)
                            .size(ButtonSize::Large);
                        if open.show(ui).clicked() {
                            events.push(ReportEvent::CheckLines(LinesToCheck::Flagged));
                        }
                    });
                });
            });
        let rows = lines.groups.len();
        for (i, (group, count)) in lines.groups.iter().enumerate() {
            group_row_ui(ui, *group, *count, i + 1 == rows, events);
        }
    });
}

/// The row of `group` with its `count` of lines; `last` rounds its hover to the card's corners.
fn group_row_ui(
    ui: &mut Ui,
    group: LineGroup,
    count: usize,
    last: bool,
    events: &mut Vec<ReportEvent>,
) {
    let p = palette(ui);
    let width = ui.available_width();
    let count_text = ui.painter().layout_no_wrap(
        count.to_string(),
        FontId::new(13.0, FontFamily::Name(fonts::SEMIBOLD.into())),
        p.text2,
    );
    let chevron =
        ui.painter()
            .layout_no_wrap(icons::CARET_RIGHT.to_string(), icons::font(14.0), p.text3);
    let text_left = ROW_SIDE + ROW_ICON + ROW_GAP;
    let text_width =
        (width - text_left - ROW_GAP - count_text.size().x - ROW_GAP - chevron.size().x - ROW_SIDE)
            .max(40.0);
    // The mockup's weight 500 falls between the fonts' 400 and 600: the title takes semibold.
    let semibold = FontId::new(13.0, FontFamily::Name(fonts::SEMIBOLD.into()));
    let title = wrapped(ui, group.title(), semibold, p.text, text_width);
    let plain = FontId::proportional(12.0);
    let explanation = wrapped(ui, group.explanation(), plain, p.text2, text_width);
    let height = 2.0 * ROW_TOP + title.size().y + 1.0 + explanation.size().y;
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, group.title()));
    if response.hovered() {
        let radius = if last {
            CornerRadius {
                sw: 9,
                se: 9,
                ..CornerRadius::ZERO
            }
        } else {
            CornerRadius::ZERO
        };
        ui.painter().rect_filled(rect, radius, p.hover);
    }
    ui.painter()
        .hline(rect.x_range(), rect.top(), Stroke::new(1.0, p.line));
    let centre = rect.center().y;
    ui.painter().text(
        pos2(rect.left() + ROW_SIDE + ROW_ICON / 2.0, centre),
        Align2::CENTER_CENTER,
        group_glyph(group),
        icons::font(ROW_ICON),
        p.warn_icon,
    );
    let top = rect.top() + ROW_TOP;
    let explanation_top = top + title.size().y + 1.0;
    ui.painter()
        .galley(pos2(rect.left() + text_left, top), title, p.text);
    ui.painter().galley(
        pos2(rect.left() + text_left, explanation_top),
        explanation,
        p.text2,
    );
    let chevron_left = rect.right() - ROW_SIDE - chevron.size().x;
    let count_left = chevron_left - ROW_GAP - count_text.size().x;
    ui.painter().galley(
        pos2(count_left, centre - count_text.size().y / 2.0),
        count_text,
        p.text2,
    );
    ui.painter().galley(
        pos2(chevron_left, centre - chevron.size().y / 2.0),
        chevron,
        p.text3,
    );
    if response.clicked() {
        events.push(ReportEvent::CheckLines(LinesToCheck::Group(group)));
    }
}

/// `text` wrapped to `width` in `font`.
fn wrapped(ui: &Ui, text: &str, font: FontId, colour: Color32, width: f32) -> Arc<Galley> {
    let job = LayoutJob::simple(text.to_string(), font, colour, width);
    ui.painter().layout_job(job)
}

/// The mark of `group`'s row.
fn group_glyph(group: LineGroup) -> &'static str {
    match group {
        LineGroup::ChangedByFixIt => icons::MAGIC_WAND,
        LineGroup::Unsure => icons::EAR,
        LineGroup::HeardWordReplaced => icons::PENCIL_SIMPLE,
        LineGroup::NovelWord => icons::INFO,
        LineGroup::TooFast => icons::CLOCK,
        LineGroup::LooselyTimed => icons::WAVEFORM,
        LineGroup::Layout => icons::SUBTITLES,
    }
}
