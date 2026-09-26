//! Lends each feature its borrowed view, draws it, and turns its events into actions.

use std::time::Instant;

use eframe::egui::{RichText, ScrollArea, Ui};

use super::{Action, TbdSubtitlesApp};
use crate::core::ui::{BAD, MUTED_TEXT};
use crate::job_queue::models::view::JobQueueView;
use crate::job_queue::ui::{progress_view_ui, queue_panel_ui};
use crate::job_report::ui::report_view_ui;
use crate::line_review::ui::{ReviewView, review_view_ui};
use crate::settings::ui::settings_page_ui;

fn queue_view(app: &TbdSubtitlesApp) -> JobQueueView<'_> {
    JobQueueView {
        queue: &app.queue,
        models_missing: app.models_missing(),
        rates: &app.rates,
        now: Instant::now(),
    }
}

/// Draw the queue panel and collect its events as actions.
pub(super) fn queue_ui(ui: &mut Ui, app: &TbdSubtitlesApp, actions: &mut Vec<Action>) {
    let mut events = Vec::new();
    queue_panel_ui(ui, &queue_view(app), &mut events);
    actions.extend(events.into_iter().map(Action::from));
}

/// Draw the selected job, or a hint when none is selected.
pub(super) fn jobs_ui(ui: &mut Ui, app: &TbdSubtitlesApp, actions: &mut Vec<Action>) {
    let Some(item) = app.queue.selected.and_then(|id| app.queue.get(id)) else {
        ui.heading(super::APP_NAME);
        ui.label(
            RichText::new("Select a job in the queue to see its progress and report.")
                .color(MUTED_TEXT),
        );
        return;
    };
    if let Some((_, session)) = app.review.as_ref().filter(|(id, _)| *id == item.id) {
        let view = ReviewView {
            session,
            playing: app.clip.as_ref().is_some_and(|clip| clip.is_playing()),
            frame: app.clip.as_ref().and_then(|clip| clip.frame()),
            job_busy: app.video_busy(item.id),
        };
        let mut events = Vec::new();
        review_view_ui(ui, &view, &mut events);
        actions.extend(events.into_iter().map(Action::from));
        return;
    }
    ScrollArea::vertical().show(ui, |ui| {
        let mut events = Vec::new();
        progress_view_ui(ui, &queue_view(app), item, &mut events);
        actions.extend(events.into_iter().map(Action::from));
        match app.report.as_ref().filter(|(id, _)| *id == item.id) {
            Some((_, Ok(report))) => {
                let mut events = Vec::new();
                report_view_ui(ui, report, &mut events);
                actions.extend(events.into_iter().map(Action::from));
            }
            Some((_, Err(error))) => {
                ui.label(RichText::new(format!("No report: {error}")).color(BAD));
            }
            None => {}
        }
    });
}

/// Draw the settings page and collect its events as actions.
pub(super) fn settings_ui(ui: &mut Ui, app: &TbdSubtitlesApp, actions: &mut Vec<Action>) {
    let mut events = Vec::new();
    settings_page_ui(ui, &app.settings, &mut events);
    actions.extend(events.into_iter().map(Action::from));
}
