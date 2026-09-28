//! The log window's content: the filter bar across the top, the lines in a scrolling list that
//! follows the newest, and the footer that counts them.
//!
//! **Role:** draw the borrowed console and turn each click and each edit of the search field into
//! a `LogConsoleEvent`; Copy puts the shown lines on the clipboard itself.
//!
//! **Position:** called by the application inside the log window's viewport; the columns come
//! from `console_text`.
//!
//! **Signals and state:** none; reads the console and returns events.
//!
//! **Invariants:** only the rows in view are laid out, so twenty thousand lines scroll smoothly;
//! a row never wraps (the list scrolls sideways); the list sticks to the newest line until the
//! owner scrolls up, and again once they scroll back down.

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    Align, CentralPanel, Color32, Frame, Id, Label, Layout, Margin, Panel, RichText, ScrollArea,
    TextEdit, TextStyle, TextWrapMode, Ui,
};
use tracing::Level;

use crate::core::format::plural;
use crate::core::log_buffer::{CAPACITY, LogLine};
use crate::core::ui::button::Button;
use crate::core::ui::icons;
use crate::core::ui::palette::{Palette, palette};
use crate::core::ui::segmented::{Tally, segmented};
use crate::log_console::events::LogConsoleEvent;
use crate::log_console::models::console::Console;
use crate::log_console::services::console_text;

/// The search field's width.
const SEARCH_WIDTH: f32 = 220.0;
/// The space between two rows.
const ROW_GAP: f32 = 2.0;

/// Draw the log window's content and push what the owner asked for onto `events`; `log_file`
/// says whether there is a log file to open.
pub(crate) fn console_window_ui(
    ui: &mut Ui,
    console: &Console,
    log_file: bool,
    events: &mut Vec<LogConsoleEvent>,
) {
    let p = palette(ui);
    Panel::top(Id::new("log-bar"))
        .frame(
            Frame::new()
                .fill(p.toolbar)
                .inner_margin(Margin::symmetric(12, 8)),
        )
        .show(ui, |ui| bar_ui(ui, console, log_file, events));
    Panel::bottom(Id::new("log-footer"))
        .frame(
            Frame::new()
                .fill(p.toolbar)
                .inner_margin(Margin::symmetric(12, 6)),
        )
        .show(ui, |ui| footer_ui(ui, console));
    CentralPanel::default()
        .frame(
            Frame::new()
                .fill(p.window)
                .inner_margin(Margin::symmetric(10, 6)),
        )
        .show(ui, |ui| lines_ui(ui, console));
}

/// The levels with the counts of errors and warnings, the search field, then Copy, Clear and
/// Open Log File on the right.
fn bar_ui(ui: &mut Ui, console: &Console, log_file: bool, events: &mut Vec<LogConsoleEvent>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (errors, warnings) = console.problems();
        let segments = [
            (
                Level::ERROR,
                "Errors",
                (errors > 0).then_some(Tally::Count(errors)),
            ),
            (
                Level::WARN,
                "Warnings",
                (warnings > 0).then_some(Tally::Count(warnings)),
            ),
            (Level::INFO, "Info", None),
            (Level::DEBUG, "Debug", None),
        ];
        if let Some(level) = segmented(ui, console.filter().level, &segments) {
            events.push(LogConsoleEvent::Level(level));
        }
        let mut search = console.search().to_string();
        let field = TextEdit::singleline(&mut search)
            .id(Id::new("log-search"))
            .hint_text(format!("{}  Filter lines", icons::MAGNIFYING_GLASS))
            .desired_width(SEARCH_WIDTH);
        if ui.add(field).changed() {
            events.push(LogConsoleEvent::Search(search));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if Button::new("Open Log File")
                .icon(icons::FILE_TEXT)
                .enabled(log_file)
                .show(ui)
                .on_hover_text("The whole log of this window, in the desktop's text editor")
                .clicked()
            {
                events.push(LogConsoleEvent::OpenLogFile);
            }
            if Button::new("Clear")
                .icon(icons::TRASH)
                .show(ui)
                .on_hover_text("Clear the lines shown here; the log file keeps them")
                .clicked()
            {
                events.push(LogConsoleEvent::Clear);
            }
            if Button::new("Copy")
                .icon(icons::COPY)
                .enabled(console.shown_len() > 0)
                .show(ui)
                .on_hover_text("Copy the lines shown")
                .clicked()
            {
                ui.ctx().copy_text(console_text::copy_text(console));
            }
        });
    });
}

/// The shown lines, only the rows in view laid out, following the newest.
fn lines_ui(ui: &mut Ui, console: &Console) {
    let p = palette(ui);
    if console.shown_len() == 0 {
        let words = if console.len() == 0 {
            "Nothing is logged yet."
        } else {
            "No line matches the filter."
        };
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new(words).color(p.text3));
        });
        return;
    }
    ui.spacing_mut().item_spacing.y = ROW_GAP;
    let row_height = ui.text_style_height(&TextStyle::Monospace);
    ScrollArea::both()
        .id_salt("log-lines")
        .auto_shrink(false)
        .stick_to_bottom(true)
        .show_rows(ui, row_height, console.shown_len(), |ui, rows| {
            for row in rows {
                if let Some(line) = console.shown_line(row) {
                    ui.add(
                        Label::new(row_job(ui, p, line))
                            .wrap_mode(TextWrapMode::Extend)
                            .selectable(true),
                    );
                }
            }
        });
}

/// One row: the time, the level in its colour, the source, then the message.
fn row_job(ui: &Ui, p: &Palette, line: &LogLine) -> LayoutJob {
    let font = TextStyle::Monospace.resolve(ui.style());
    let (level_colour, message_colour) = colours(p, line.level);
    let mut job = LayoutJob::default();
    let mut add = |text: &str, colour: Color32| {
        job.append(text, 0.0, TextFormat::simple(font.clone(), colour));
    };
    add(&console_text::time(line.elapsed), p.text3);
    add("  ", p.text3);
    add(&console_text::level(line.level), level_colour);
    add("  ", p.text3);
    add(&console_text::source(&line.target), p.text2);
    add("  ", p.text3);
    add(&line.message, message_colour);
    job
}

/// The colours of a line's level and of its message.
fn colours(p: &Palette, level: Level) -> (Color32, Color32) {
    match level {
        Level::ERROR => (p.bad, p.bad),
        Level::WARN => (p.warn, p.text),
        Level::INFO => (p.accent, p.text),
        _ => (p.text3, p.text2),
    }
}

/// How many lines show of how many kept, and what the window keeps.
fn footer_ui(ui: &mut Ui, console: &Console) {
    let p = palette(ui);
    let counted = if console.shown_len() == console.len() {
        plural(console.len(), "line")
    } else {
        format!(
            "{} of {}",
            console.shown_len(),
            plural(console.len(), "line")
        )
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(counted).size(11.5).color(p.text2));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(format!(
                    "Times since the window opened · keeps the newest {CAPACITY} lines"
                ))
                .size(11.5)
                .color(p.text3),
            );
        });
    });
}
