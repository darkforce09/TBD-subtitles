//! The detail pane's header: the selected job's name as the page title, the line under it, and
//! on the right the Overview | Check Lines switch of a finished job (with its count of lines to
//! check, or a check once none is left) or the Cancel of a running one; and the hint while no job
//! is selected.
//!
//! **Role:** draw the header from the borrowed state and ask for the tab or the cancel as
//! actions.
//!
//! **Position:** called by `feature_views::jobs_ui` above the selected job's body; writes a
//! finished job's line from its report, any other job's from `job_queue::services::status_text`.
//!
//! **Signals and state:** none; the tab shown is derived from the loaded line review.
//!
//! **Invariants:** Check Lines shows exactly while a line review of the selected job is loaded; a
//! running job that is stopping shows "Stopping…" in place of Cancel.

use std::time::Instant;

use eframe::egui::{
    Align, CornerRadius, FontFamily, FontId, Frame, Label, Layout, Margin, Rect, RichText, Sense,
    TextStyle, Ui, UiBuilder, pos2, vec2,
};

use super::{Action, TbdSubtitlesApp};
use crate::core::format;
use crate::core::ui::button::Button;
use crate::core::ui::fonts;
use crate::core::ui::icons::{self, StatusIcon, status_icon};
use crate::core::ui::palette::palette;
use crate::core::ui::segmented::{Tally, segmented};
use crate::core::ui::theme::TITLE;
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::queue::{JobId, JobState, QueueItem};
use crate::job_queue::services::status_text;

/// The header's content height: the title over its line.
const HEIGHT: f32 = 48.0;
/// The space between the title and what stands on the right.
const GAP: f32 = 16.0;

/// The two views of a finished job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DetailTab {
    /// Its report.
    Overview,
    /// Its lines to check: the line review.
    CheckLines,
}

impl TbdSubtitlesApp {
    /// The tab job `id` shows: Check Lines exactly while its line review is loaded.
    pub(super) fn detail_tab(&self, id: JobId) -> DetailTab {
        if self.review.as_ref().is_some_and(|(job, _)| *job == id) {
            DetailTab::CheckLines
        } else {
            DetailTab::Overview
        }
    }
}

/// Draw the header of the selected job `item`.
pub(super) fn header_ui(
    ui: &mut Ui,
    app: &TbdSubtitlesApp,
    item: &QueueItem,
    actions: &mut Vec<Action>,
) {
    let p = palette(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), HEIGHT), Sense::hover());
    let mut right = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::right_to_left(Align::Center)),
    );
    right_ui(&mut right, app, item, actions);
    let used = right.min_rect();
    let title_right = if used.width() > 0.0 {
        used.left() - GAP
    } else {
        rect.right()
    };
    let mut left = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_max(
                rect.min,
                pos2(title_right, rect.bottom()),
            ))
            .layout(Layout::top_down(Align::Min)),
    );
    left.spacing_mut().item_spacing.y = 2.0;
    left.add(
        Label::new(
            RichText::new(item.name())
                .text_style(TextStyle::Name(TITLE.into()))
                .color(p.text),
        )
        .truncate(),
    );
    left.add(Label::new(RichText::new(line(app, item)).size(12.0).color(p.text2)).truncate());
}

/// What stands on the right: the tabs of a finished job, Cancel or "Stopping…" for a running one.
fn right_ui(ui: &mut Ui, app: &TbdSubtitlesApp, item: &QueueItem, actions: &mut Vec<Action>) {
    match &item.state {
        JobState::Finished(_) | JobState::FinishedBefore => {
            let tally = app
                .summaries
                .get(&item.id)
                .filter(|summary| summary.flagged > 0)
                .map(|summary| match summary.to_check {
                    0 => Tally::Done,
                    n => Tally::Count(n),
                });
            let tabs = [
                (DetailTab::Overview, "Overview", None),
                (DetailTab::CheckLines, "Check Lines", tally),
            ];
            if let Some(tab) = segmented(ui, app.detail_tab(item.id), &tabs) {
                actions.push(Action::ShowTab(tab));
            }
        }
        JobState::Running(progress) if progress.cancelling => stopping_ui(ui),
        JobState::Running(_) => {
            let cancel = Button::new("Cancel")
                .icon(icons::STOP_CIRCLE)
                .danger(true)
                .show(ui);
            if cancel.clicked() {
                actions.push(Action::from(JobQueueEvent::Cancel(item.id)));
            }
        }
        JobState::Waiting | JobState::Failed(_) | JobState::Cancelled { .. } => {}
    }
}

/// The grey pill with a spinner that stands in for Cancel while a job stops.
fn stopping_ui(ui: &mut Ui) {
    let p = palette(ui);
    Frame::new()
        .fill(p.seg_bg)
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                status_icon(ui, StatusIcon::Working, 14.0, false);
                let semibold = FontFamily::Name(fonts::SEMIBOLD.into());
                ui.label(
                    RichText::new("Stopping…")
                        .font(FontId::new(11.5, semibold))
                        .color(p.text2),
                );
            });
        });
}

/// The line under the title: a finished job's length and the time its steps took, from its
/// report; any other job's from its state.
fn line(app: &TbdSubtitlesApp, item: &QueueItem) -> String {
    let finished = matches!(item.state, JobState::Finished(_) | JobState::FinishedBefore);
    match app.report.as_ref().filter(|(id, _)| *id == item.id) {
        Some((_, Ok(report))) if finished => format!(
            "{} video · finished in {}",
            format::length(report.qc.summary.video_s),
            format::duration(report.total_s())
        ),
        _ => status_text::detail_line(item, &app.queue, Instant::now()),
    }
}

/// The middle of the pane while no job is selected.
pub(super) fn hint_ui(ui: &mut Ui) {
    let p = palette(ui);
    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.add_space((ui.available_height() / 2.0 - 40.0).max(0.0));
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.label(
            RichText::new(icons::LIST)
                .font(icons::font(28.0))
                .color(p.text3),
        );
        ui.label(
            RichText::new("Select a video")
                .text_style(TextStyle::Heading)
                .color(p.text),
        );
        ui.label(
            RichText::new("Its progress, subtitles and lines to check show here.").color(p.text2),
        );
    });
}
