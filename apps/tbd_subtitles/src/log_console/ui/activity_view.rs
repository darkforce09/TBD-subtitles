//! The activity list: one row per line (the time, a chip saying who wrote it, the message cut
//! to the width), and a header row each time the work moves to another video or step.
//!
//! **Role:** draw the rows of the borrowed `Activity` in view, and turn a click on a line into
//! `SelectLine` (again on the open line closes it).
//!
//! **Position:** drawn by `console_window` in the central panel while the activity view shows.
//!
//! **Signals and state:** none; reads the activity and returns events.
//!
//! **Invariants:** every row has one height, so only the rows in view are laid out; nothing
//! scrolls sideways (a long message ends in `…`, and the detail panel shows it whole); the list
//! sticks to the newest row until the owner scrolls up.

use eframe::egui::text::{LayoutJob, TextFormat, TextWrapping};
use eframe::egui::{
    Align2, Color32, CornerRadius, FontFamily, FontId, Rect, RichText, ScrollArea, Sense,
    TextStyle, Ui, WidgetInfo, WidgetType, pos2, vec2,
};
use tracing::Level;

use crate::core::log_buffer::LogLine;
use crate::core::ui::fonts;
use crate::core::ui::icons;
use crate::core::ui::palette::{Palette, palette};
use crate::log_console::events::LogConsoleEvent;
use crate::log_console::models::activity::{Activity, RowView};
use crate::log_console::models::who::Who;
use crate::log_console::services::console_text;

/// A row's height.
const ROW: f32 = 22.0;
/// The widths of the time column and of a chip, and the space between columns.
const TIME: f32 = 78.0;
const CHIP: f32 = 58.0;
const GAP: f32 = 10.0;

/// Draw the list, or why it is empty.
pub(crate) fn activity_ui(ui: &mut Ui, activity: &Activity, events: &mut Vec<LogConsoleEvent>) {
    let p = palette(ui);
    if activity.rows_len() == 0 {
        let words = if activity.len() == 0 {
            "Nothing is logged yet."
        } else {
            "No line matches the filter."
        };
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new(words).color(p.text3));
        });
        return;
    }
    ui.spacing_mut().item_spacing.y = 0.0;
    let selected = activity.selected().map(|line| line.seq);
    ScrollArea::vertical()
        .id_salt("log-lines")
        .auto_shrink(false)
        .stick_to_bottom(true)
        .show_rows(ui, ROW, activity.rows_len(), |ui, rows| {
            for index in rows {
                match activity.row(index) {
                    Some(RowView::Header { video, step }) => header_ui(ui, p, video, step),
                    Some(RowView::Line(line)) => {
                        let open = selected == Some(line.seq);
                        if line_ui(ui, p, line, open) {
                            events.push(LogConsoleEvent::SelectLine((!open).then_some(line.seq)));
                        }
                    }
                    None => {}
                }
            }
        });
}

/// A group's header: the video and the step in words, over a hairline.
fn header_ui(ui: &mut Ui, p: &Palette, video: Option<&str>, step: Option<&str>) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::hover());
    ui.painter()
        .hline(rect.x_range(), rect.top() + 2.0, (1.0, p.line));
    let text = console_text::header(video, step);
    let font = FontId::new(12.5, FontFamily::Name(fonts::SEMIBOLD.into()));
    let galley = one_line(ui, &text, font, p.text, rect.width() - 8.0);
    ui.painter().galley(
        pos2(
            rect.left() + 4.0,
            rect.center().y + 1.0 - galley.size().y / 2.0,
        ),
        galley,
        p.text,
    );
}

/// One line: its time, its writer's chip, a mark when it is a warning or an error, and the
/// message cut to the width; whether it was clicked.
fn line_ui(ui: &mut Ui, p: &Palette, line: &LogLine, open: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &line.message));
    if open {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), p.accent_tint);
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), p.hover);
    }
    let mono = TextStyle::Monospace.resolve(ui.style());
    let centre = rect.center().y;
    ui.painter().text(
        pos2(rect.left() + 4.0, centre),
        Align2::LEFT_CENTER,
        console_text::time(line.elapsed),
        mono.clone(),
        p.text3,
    );
    let chip = Rect::from_center_size(
        pos2(rect.left() + 4.0 + TIME + CHIP / 2.0, centre),
        vec2(CHIP, 16.0),
    );
    chip_ui(ui, p, chip, Who::of(&line.target));
    let mut left = chip.right() + GAP;
    let (mark, colour) = colours(p, line.level);
    if let Some(mark) = mark {
        ui.painter().text(
            pos2(left, centre),
            Align2::LEFT_CENTER,
            icons::WARNING,
            icons::font(13.0),
            mark,
        );
        left += 18.0;
    }
    let galley = one_line(ui, &line.message, mono, colour, rect.right() - left - 4.0);
    ui.painter()
        .galley(pos2(left, centre - galley.size().y / 2.0), galley, colour);
    response.clicked()
}

/// The chip of `who` in `rect`: the AI in blue, jobs in green, programs grey, the app outlined.
fn chip_ui(ui: &Ui, p: &Palette, rect: Rect, who: Who) {
    let (fill, text) = match who {
        Who::Ai => (p.accent_tint, p.accent),
        Who::Job => (p.good_tint, p.good),
        Who::Program => (p.seg_bg, p.text2),
        Who::App => (Color32::TRANSPARENT, p.text3),
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(8), fill);
    if who == Who::App {
        painter.rect_stroke(
            rect,
            CornerRadius::same(8),
            (1.0, p.line_strong),
            eframe::egui::StrokeKind::Inside,
        );
    }
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        who.label(),
        FontId::new(10.5, FontFamily::Name(fonts::SEMIBOLD.into())),
        text,
    );
}

/// The mark's colour (warnings and errors only) and the message's colour for `level`.
fn colours(p: &Palette, level: Level) -> (Option<Color32>, Color32) {
    match level {
        Level::ERROR => (Some(p.bad), p.bad),
        Level::WARN => (Some(p.warn), p.text),
        Level::INFO => (None, p.text),
        _ => (None, p.text2),
    }
}

/// `text` on one line at most `width` wide, ending in `…` when cut.
pub(super) fn one_line(
    ui: &Ui,
    text: &str,
    font: FontId,
    colour: Color32,
    width: f32,
) -> std::sync::Arc<eframe::egui::Galley> {
    let mut job = LayoutJob::single_section(text.to_string(), TextFormat::simple(font, colour));
    job.wrap = TextWrapping::truncate_at_width(width.max(0.0));
    ui.painter().layout_job(job)
}
