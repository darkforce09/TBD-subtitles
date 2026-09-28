//! The log window's content: the bar with the view switch and the buttons, the filters, the view
//! shown (the activity list or the model calls), and the footer that counts them.
//!
//! **Role:** draw the borrowed console and turn each click and each edit of the search field into
//! a `LogConsoleEvent`; Copy puts the shown lines, or the open call, on the clipboard itself.
//!
//! **Position:** called by the application inside the log window's viewport; draws
//! `activity_view`, `line_detail` and `calls_view`.
//!
//! **Signals and state:** none; reads the console and returns events.
//!
//! **Invariants:** one search field filters both views; Clear clears the view shown only; the
//! level and writer filters show only with the activity list.

use eframe::egui::{Align, CentralPanel, Frame, Id, Layout, Margin, Panel, RichText, TextEdit, Ui};
use tracing::Level;

use super::{activity_view, calls_view, line_detail};
use crate::core::format::plural;
use crate::core::log_buffer::{CALL_CAPACITY, CAPACITY};
use crate::core::ui::button::Button;
use crate::core::ui::icons;
use crate::core::ui::palette::palette;
use crate::core::ui::segmented::{Tally, segmented};
use crate::log_console::events::LogConsoleEvent;
use crate::log_console::models::console::{ConsoleView, LogConsole};
use crate::log_console::models::who::Who;
use crate::log_console::services::{call_text, console_text};

/// The search field's width.
const SEARCH_WIDTH: f32 = 240.0;

/// Draw the log window's content and push what the owner asked for onto `events`; `log_file`
/// says whether there is a log file to open.
pub(crate) fn console_window_ui(
    ui: &mut Ui,
    console: &LogConsole,
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
        .show(ui, |ui| {
            bar_ui(ui, console, log_file, events);
            ui.add_space(6.0);
            filters_ui(ui, console, events);
        });
    Panel::bottom(Id::new("log-footer"))
        .frame(
            Frame::new()
                .fill(p.toolbar)
                .inner_margin(Margin::symmetric(12, 6)),
        )
        .show(ui, |ui| footer_ui(ui, console));
    match console.view {
        ConsoleView::Activity => {
            if let Some(line) = console.activity.selected() {
                line_detail::line_detail_ui(ui, line, events);
            }
            CentralPanel::default()
                .frame(
                    Frame::new()
                        .fill(p.window)
                        .inner_margin(Margin::symmetric(8, 6)),
                )
                .show(ui, |ui| {
                    activity_view::activity_ui(ui, &console.activity, events)
                });
        }
        ConsoleView::Calls => calls_view::calls_ui(ui, &console.calls, events),
    }
}

/// The view switch on the left; Copy, Clear and Open Log File on the right.
fn bar_ui(ui: &mut Ui, console: &LogConsole, log_file: bool, events: &mut Vec<LogConsoleEvent>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let views = [
            (ConsoleView::Activity, "Activity", None),
            (
                ConsoleView::Calls,
                "Model Calls",
                Some(Tally::Count(console.calls.len())),
            ),
        ];
        if let Some(view) = segmented(ui, console.view, &views) {
            events.push(LogConsoleEvent::View(view));
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
            let clear_hint = match console.view {
                ConsoleView::Activity => "Clear the lines shown here; the log file keeps them",
                ConsoleView::Calls => "Clear the model calls kept here",
            };
            if Button::new("Clear")
                .icon(icons::TRASH)
                .show(ui)
                .on_hover_text(clear_hint)
                .clicked()
            {
                events.push(LogConsoleEvent::Clear);
            }
            copy_ui(ui, console);
        });
    });
}

/// Copy: the shown lines, or the open call whole.
fn copy_ui(ui: &mut Ui, console: &LogConsole) {
    let (enabled, hint) = match console.view {
        ConsoleView::Activity => (console.activity.shown_len() > 0, "Copy the lines shown"),
        ConsoleView::Calls => (
            console.calls.selected().is_some(),
            "Copy the open call: its prompt, message, schema and answer",
        ),
    };
    if Button::new("Copy")
        .icon(icons::COPY)
        .enabled(enabled)
        .show(ui)
        .on_hover_text(hint)
        .clicked()
    {
        let text = match console.view {
            ConsoleView::Activity => console_text::copy_text(&console.activity),
            ConsoleView::Calls => console
                .calls
                .selected()
                .map(call_text::call_text)
                .unwrap_or_default(),
        };
        ui.ctx().copy_text(text);
    }
}

/// The levels and writers (activity only), then the search field.
fn filters_ui(ui: &mut Ui, console: &LogConsole, events: &mut Vec<LogConsoleEvent>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if console.view == ConsoleView::Activity {
            let activity = &console.activity;
            let (errors, warnings) = activity.problems();
            let levels = [
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
            if let Some(level) = segmented(ui, activity.filter().level, &levels) {
                events.push(LogConsoleEvent::Level(level));
            }
            let mut writers = vec![(None, "Everyone", None)];
            writers.extend(Who::ALL.map(|who| (Some(who), who.plural(), None)));
            if let Some(who) = segmented(ui, activity.filter().who, &writers) {
                events.push(LogConsoleEvent::Who(who));
            }
        }
        let mut search = console.search().to_string();
        let hint = match console.view {
            ConsoleView::Activity => "Filter lines",
            ConsoleView::Calls => "Filter calls",
        };
        let field = TextEdit::singleline(&mut search)
            .id(Id::new("log-search"))
            .hint_text(format!("{}  {hint}", icons::MAGNIFYING_GLASS))
            .desired_width(SEARCH_WIDTH);
        if ui.add(field).changed() {
            events.push(LogConsoleEvent::Search(search));
        }
    });
}

/// How many lines or calls show of how many kept, and what the window keeps.
fn footer_ui(ui: &mut Ui, console: &LogConsole) {
    let p = palette(ui);
    let (shown, kept, noun, keeps) = match console.view {
        ConsoleView::Activity => (
            console.activity.shown_len(),
            console.activity.len(),
            "line",
            format!("Times since the window opened · keeps the newest {CAPACITY} lines"),
        ),
        ConsoleView::Calls => (
            console.calls.shown_len(),
            console.calls.len(),
            "call",
            format!("Kept in memory only · the newest {CALL_CAPACITY} calls"),
        ),
    };
    let mut counted = if shown == kept {
        plural(kept, noun)
    } else {
        format!("{shown} of {}", plural(kept, noun))
    };
    let failed = console.calls.failed();
    if console.view == ConsoleView::Calls && failed > 0 {
        counted.push_str(&format!(" · {failed} failed"));
    }
    ui.horizontal(|ui| {
        ui.label(RichText::new(counted).size(11.5).color(p.text2));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(keeps).size(11.5).color(p.text3));
        });
    });
}
