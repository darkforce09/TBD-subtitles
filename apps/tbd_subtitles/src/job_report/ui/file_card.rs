//! The Overview's file card: the subtitles saved next to the video, whether they pass the quality
//! check, what Fix It did, Fix It under way, the correction run under way, each problem in plain
//! words with its fix, the Fix It row, the path, and the buttons that open the video, show the
//! file and copy its path.
//!
//! **Role:** draw the borrowed report's verdict, Fix It's result and the problems and turn the
//! buttons into `ReportEvent`s.
//!
//! **Position:** the first card of `overview`; its head is `overview::head_ui`, its Fix It result
//! `fix_result_card`, which borrows the notes and the problems' buttons from here.
//!
//! **Signals and state:** none, but for copying the subtitle path to the clipboard.
//!
//! **Invariants:** the pill says "Passes the quality check" exactly when there is no problem; a
//! problem's button is drawn only when it has a remedy; each problem is drawn once, in Fix It's
//! result while it shows, else on its own; Try Again reruns the language-model calls, never the
//! steps before them; Fix It's result shows only while Fix It is not under way on the video; the
//! Fix It row shows only when Fix It has findings to ask about, its button off with why while it
//! cannot run, and Stop while it runs; while Fix It's correction run waits or runs, Fix It's note
//! says so in place of the correction note; the path shows its folder and file, the whole path on
//! hover.

use std::path::Path;

use eframe::egui::{
    Align, Color32, CornerRadius, FontFamily, FontId, Frame, Label, Layout, Margin, RichText, Ui,
};

use crate::core::format;
use crate::core::ui::button::{Button, ButtonSize};
use crate::core::ui::card::{card, well_text};
use crate::core::ui::fonts;
use crate::core::ui::icons::{self, StatusIcon, status_icon};
use crate::core::ui::palette::palette;
use crate::core::ui::pill::{Tone, pill};
use crate::job_report::events::{LinesToCheck, ReportEvent};
use crate::job_report::models::finding_group::LineGroup;
use crate::job_report::models::fixing::{FixView, stage_words, updating_words};
use crate::job_report::models::problem::{Problem, Remedy};
use crate::job_report::ui::fix_result_card::fix_result_ui;
use crate::job_report::ui::overview::{OverviewView, head_ui};

/// The space between a note's mark and its text, and the space inside it.
const NOTE_GAP: f32 = 10.0;
const NOTE_MARGIN: Margin = Margin {
    left: 12,
    right: 12,
    top: 10,
    bottom: 10,
};

/// Draw the file card of `view`'s report, with its correction run and Fix It.
pub(super) fn file_card_ui(ui: &mut Ui, view: &OverviewView<'_>, events: &mut Vec<ReportEvent>) {
    let (report, fix) = (view.report, &view.fix);
    let p = palette(ui);
    let passes = report.problems.is_empty();
    let result = report.fix_result.as_ref().filter(|_| !fix.under_way());
    card(ui, true, |ui| {
        let (colour, line) = if passes {
            (
                p.good_icon,
                "The file already holds the app's best reading of every line, and it passes the \
                 quality check.",
            )
        } else {
            (
                p.warn_icon,
                "The file holds the app's best reading of every line, but the quality check \
                 found problems.",
            )
        };
        head_ui(
            ui,
            (icons::SUBTITLES, colour),
            "Subtitles saved next to the video",
            line,
            |ui| {
                if passes {
                    pill(ui, Tone::Good, icons::CHECK, "Passes the quality check");
                } else {
                    pill(ui, Tone::Warn, icons::WARNING, "Needs attention");
                }
            },
        );
        if let Some(result) = result {
            let unchecked = report.corrections.unchecked_fix_count();
            fix_result_ui(ui, result, unchecked, view.fixed_at, events);
        }
        fixing_ui(ui, fix, events);
        if let Some(corrections) = view.updating
            && !matches!(fix, FixView::Updating { .. })
        {
            updating_ui(ui, corrections);
        }
        if result.is_none() {
            for problem in &report.problems {
                problem_ui(ui, *problem, events);
            }
        }
        fix_it_ui(ui, fix, !passes, events);
        let full = report.subtitles.display().to_string();
        ui.scope(|ui| well_text(ui, &short_path(&report.subtitles), false))
            .response
            .on_hover_text(full);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let open = Button::new("Open in Player").icon(icons::MONITOR_PLAY);
            if open.show(ui).clicked() {
                events.push(ReportEvent::OpenVideo(report.video.clone()));
            }
            let folder = Button::new("Show in Folder").icon(icons::FOLDER);
            if folder.show(ui).clicked() {
                events.push(ReportEvent::ShowInFolder(report.subtitles.clone()));
            }
            if Button::new("Copy Path")
                .icon(icons::COPY)
                .show(ui)
                .clicked()
            {
                ui.ctx().copy_text(report.subtitles.display().to_string());
                events.push(ReportEvent::Copied);
            }
        });
    });
}

/// `…/<folder>/<file>`: the file and the folder that holds it, as the mockup shows the path; the
/// whole path when it has no folder.
fn short_path(path: &Path) -> String {
    match (path.parent().and_then(Path::file_name), path.file_name()) {
        (Some(folder), Some(file)) => {
            format!("…/{}/{}", folder.to_string_lossy(), file.to_string_lossy())
        }
        _ => path.display().to_string(),
    }
}

/// The blue note while a correction run puts `corrections` into the file.
fn updating_ui(ui: &mut Ui, corrections: usize) {
    let p = palette(ui);
    let title = format!(
        "Updating subtitles with {}…",
        format::plural(corrections, "correction")
    );
    let line = "Each corrected line is timed again, then the file is rewritten. This takes a few \
                seconds.";
    let body = |ui: &mut Ui| {
        ui.add(Label::new(RichText::new(line).size(12.0).color(p.text2)).wrap());
    };
    let mark = |ui: &mut Ui| status_icon(ui, StatusIcon::Working, 18.0, false);
    note_ui(ui, p.accent_tint, mark, &title, |_| {}, body);
}

/// The blue note while Fix It runs on the video: its model, its step, and Stop; or, while its
/// correction run waits or runs, its last step.
fn fixing_ui(ui: &mut Ui, fix: &FixView, events: &mut Vec<ReportEvent>) {
    let (model, stage, done, total, stopping) = match fix {
        FixView::Running {
            model,
            stage,
            done,
            total,
            stopping,
        } => (model, stage, done, total, stopping),
        FixView::Updating { model } => return fix_updating_ui(ui, model),
        FixView::Hidden | FixView::Ready { .. } | FixView::Unavailable { .. } => return,
    };
    let p = palette(ui);
    let title = format!("Fixing with {model} · {}…", stage_words(*stage));
    let line = if *stopping {
        "Stopping. Nothing is changed; Fix It again picks up where it stopped.".to_string()
    } else {
        format!(
            "{done} of {} done. The subtitles change only once every change is checked.",
            format::plural(*total, "call")
        )
    };
    let body = |ui: &mut Ui| {
        ui.add(Label::new(RichText::new(line).size(12.0).color(p.text2)).wrap());
    };
    let stop = |ui: &mut Ui| {
        let label = if *stopping { "Stopping…" } else { "Stop" };
        let button = Button::new(label)
            .icon(icons::STOP)
            .size(ButtonSize::Small)
            .enabled(!stopping);
        if button.show(ui).clicked() {
            events.push(ReportEvent::StopFix);
        }
    };
    let mark = |ui: &mut Ui| status_icon(ui, StatusIcon::Working, 18.0, false);
    note_ui(ui, p.accent_tint, mark, &title, stop, body);
}

/// The blue note while Fix It's correction run puts `model`'s changes into the subtitles.
fn fix_updating_ui(ui: &mut Ui, model: &str) {
    let p = palette(ui);
    let title = format!("Fixing with {model} · {}…", updating_words());
    let line = "Each changed line is timed again, then the file is rewritten. This takes a few \
                seconds.";
    let body = |ui: &mut Ui| {
        ui.add(Label::new(RichText::new(line).size(12.0).color(p.text2)).wrap());
    };
    let mark = |ui: &mut Ui| status_icon(ui, StatusIcon::Working, 18.0, false);
    note_ui(ui, p.accent_tint, mark, &title, |_| {}, body);
}

/// The Fix It row: what Fix It does and its button, primary when the job `needs_attention`, off
/// with why when it cannot run now.
fn fix_it_ui(ui: &mut Ui, fix: &FixView, needs_attention: bool, events: &mut Vec<ReportEvent>) {
    let (model, reason) = match fix {
        FixView::Ready { model } => (model, None),
        FixView::Unavailable { model, reason } => (model, Some(reason)),
        FixView::Hidden | FixView::Running { .. } | FixView::Updating { .. } => return,
    };
    let p = palette(ui);
    let line = format!(
        "{model} reads the whole video, fixes the flagged lines and checks each change. You can \
         keep or undo every change in Check Lines."
    );
    let body = |ui: &mut Ui| {
        ui.add(Label::new(RichText::new(line).size(12.0).color(p.text2)).wrap());
        if let Some(reason) = reason {
            ui.add(Label::new(RichText::new(reason.as_str()).size(12.0).color(p.warn)).wrap());
        }
    };
    let button = |ui: &mut Ui| {
        let fix_it = Button::new("Fix It")
            .icon(icons::MAGIC_WAND)
            .primary(needs_attention)
            .enabled(reason.is_none())
            .show(ui);
        let fix_it = match reason {
            Some(reason) => fix_it.on_disabled_hover_text(reason.as_str()),
            None => fix_it,
        };
        if fix_it.clicked() {
            events.push(ReportEvent::FixIt);
        }
    };
    let wand = |ui: &mut Ui| {
        ui.label(
            RichText::new(icons::MAGIC_WAND)
                .font(icons::font(18.0))
                .color(p.accent),
        )
    };
    note_ui(
        ui,
        p.well,
        wand,
        &format!("Fix It with {model}"),
        button,
        body,
    );
}

/// One problem on the recessed well: its warning mark, its title and fix, and its button.
fn problem_ui(ui: &mut Ui, problem: Problem, events: &mut Vec<ReportEvent>) {
    let p = palette(ui);
    let button = |ui: &mut Ui| remedy_button(ui, problem, events);
    let mark = |ui: &mut Ui| status_icon(ui, StatusIcon::Warning, 18.0, false);
    note_ui(ui, p.well, mark, &problem.title(), button, |ui| {
        ui.add(Label::new(RichText::new(problem.fix()).size(12.0).color(p.text2)).wrap());
    });
}

/// The small button of `problem`'s remedy, when it has one.
pub(super) fn remedy_button(ui: &mut Ui, problem: Problem, events: &mut Vec<ReportEvent>) {
    let Some(remedy) = problem.remedy() else {
        return;
    };
    let mut button = Button::new(remedy.label()).size(ButtonSize::Small);
    if remedy == Remedy::TryAgain {
        button = button.icon(icons::ARROW_CLOCKWISE);
    }
    if button.show(ui).clicked() {
        events.push(match remedy {
            Remedy::TryAgain => ReportEvent::TryAgain,
            Remedy::ShowNearbyLines(at) => ReportEvent::CheckLines(LinesToCheck::Near(at)),
            Remedy::ShowTooFastLines => {
                ReportEvent::CheckLines(LinesToCheck::Group(LineGroup::TooFast))
            }
        });
    }
}

/// A note on `fill`, radius 8: the 18 px mark `mark` draws, `title` in semibold over what `body`
/// draws, and what `aside` draws at the top right.
pub(super) fn note_ui<R>(
    ui: &mut Ui,
    fill: Color32,
    mark: impl FnOnce(&mut Ui) -> R,
    title: &str,
    aside: impl FnOnce(&mut Ui),
    body: impl FnOnce(&mut Ui),
) {
    let p = palette(ui);
    Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(NOTE_MARGIN)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = NOTE_GAP;
                let _ = mark(ui);
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    aside(ui);
                    ui.vertical(|ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = 2.0;
                        let semibold = FontId::new(13.0, FontFamily::Name(fonts::SEMIBOLD.into()));
                        ui.add(
                            Label::new(RichText::new(title).font(semibold).color(p.text)).wrap(),
                        );
                        body(ui);
                    });
                });
            });
        });
}
