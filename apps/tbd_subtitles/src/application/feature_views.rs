//! Lends each feature its borrowed view, draws it, and turns its events into actions.
//!
//! **Role:** build each feature's narrow view from the application state, call the feature's
//! `ui`, and wrap the events it returns as `Action`s.
//!
//! **Position:** called by `window` (toolbar, sidebar, the selected job) and by
//! `settings_window`; the only place the application calls the features' `ui` modules; the
//! selected job's header is `detail_view`'s.
//!
//! **Signals and state:** none; reads the state and pushes actions.
//!
//! **Invariants:** a feature sees only its borrowed view; with no job in the queue the right side
//! shows the empty list's card; a selected job shows its header over its cards, its report or
//! its lines to check (only while it is finished), the cards and the report in a column at most
//! 800 px wide.

use std::time::Instant;

use eframe::egui::{Frame, Id, Margin, Panel, RichText, ScrollArea, Ui};

use super::detail_view;
use super::{Action, TbdSubtitlesApp};
use crate::core::ui::palette::palette;
use crate::job_queue::models::queue::{JobState, QueueItem};
use crate::job_queue::models::view::JobQueueView;
use crate::job_queue::ui as job_queue_ui;
use crate::job_report::ui::report_view_ui;
use crate::line_review::ui::{ReviewView, review_view_ui};
use crate::settings::ui::settings_page_ui;

/// The widest the selected job's column grows, and the space between its cards.
const COLUMN_WIDTH: f32 = 800.0;
const CARD_GAP: f32 = 16.0;

fn queue_view(app: &TbdSubtitlesApp) -> JobQueueView<'_> {
    JobQueueView {
        queue: &app.queue,
        models_missing: app.models_missing(),
        rates: &app.rates,
        now: Instant::now(),
    }
}

/// Draw the toolbar and collect its events as actions; the gear opens Settings.
pub(super) fn toolbar_ui(ui: &mut Ui, app: &TbdSubtitlesApp, actions: &mut Vec<Action>) {
    let mut events = Vec::new();
    let settings = job_queue_ui::toolbar_ui(ui, &queue_view(app), &mut events);
    actions.extend(events.into_iter().map(Action::from));
    if settings {
        actions.push(Action::ShowSettings(true));
    }
}

/// Draw the sidebar and collect its events as actions.
pub(super) fn sidebar_ui(ui: &mut Ui, app: &TbdSubtitlesApp, actions: &mut Vec<Action>) {
    let mut events = Vec::new();
    job_queue_ui::sidebar_ui(ui, &queue_view(app), &mut events);
    actions.extend(events.into_iter().map(Action::from));
}

/// Draw the selected job under its header, the empty list's card, or a hint when none is
/// selected.
pub(super) fn jobs_ui(ui: &mut Ui, app: &TbdSubtitlesApp, actions: &mut Vec<Action>) {
    if app.queue.items.is_empty() {
        let mut events = Vec::new();
        job_queue_ui::empty_state_ui(ui, &queue_view(app), &mut events);
        actions.extend(events.into_iter().map(Action::from));
        return;
    }
    let Some(item) = app.queue.selected.and_then(|id| app.queue.get(id)) else {
        detail_view::hint_ui(ui);
        return;
    };
    let p = palette(ui);
    Panel::top(Id::new("detail-head"))
        .frame(Frame::new().fill(p.window).inner_margin(Margin {
            left: 24,
            right: 24,
            top: 16,
            bottom: 14,
        }))
        .show(ui, |ui| detail_view::header_ui(ui, app, item, actions));
    let finished = matches!(item.state, JobState::Finished(_) | JobState::FinishedBefore);
    if let Some((_, session)) = app
        .review
        .as_ref()
        .filter(|(id, _)| finished && *id == item.id)
    {
        let view = ReviewView {
            session,
            playing: app.clip.as_ref().is_some_and(|clip| clip.is_playing()),
            frame: app.clip.as_ref().and_then(|clip| clip.frame()),
            job_busy: app.video_busy(item.id),
        };
        Frame::new().inner_margin(Margin::same(8)).show(ui, |ui| {
            let mut events = Vec::new();
            review_view_ui(ui, &view, &mut events);
            actions.extend(events.into_iter().map(Action::from));
        });
        return;
    }
    ScrollArea::vertical()
        .id_salt(("detail", item.id))
        .auto_shrink(false)
        .show(ui, |ui| {
            Frame::new()
                .inner_margin(Margin {
                    left: 24,
                    right: 24,
                    top: 20,
                    bottom: 32,
                })
                .show(ui, |ui| {
                    ui.set_max_width(COLUMN_WIDTH);
                    ui.spacing_mut().item_spacing.y = CARD_GAP;
                    body_ui(ui, app, item, actions);
                });
        });
}

/// The selected job's body: a finished job's report, else the queue's cards for its state.
fn body_ui(ui: &mut Ui, app: &TbdSubtitlesApp, item: &QueueItem, actions: &mut Vec<Action>) {
    if !matches!(item.state, JobState::Finished(_) | JobState::FinishedBefore) {
        let mut events = Vec::new();
        job_queue_ui::progress_view_ui(ui, &queue_view(app), item, &mut events);
        actions.extend(events.into_iter().map(Action::from));
        return;
    }
    match app.report.as_ref().filter(|(id, _)| *id == item.id) {
        Some((_, Ok(report))) => {
            ui.spacing_mut().item_spacing.y = 6.0;
            let mut events = Vec::new();
            report_view_ui(ui, report, &mut events);
            actions.extend(events.into_iter().map(Action::from));
        }
        Some((_, Err(error))) => {
            ui.label(RichText::new(format!("No report: {error}")).color(palette(ui).bad));
        }
        None => {}
    }
}

/// Draw the settings page and collect its events as actions.
pub(super) fn settings_ui(ui: &mut Ui, app: &TbdSubtitlesApp, actions: &mut Vec<Action>) {
    let mut events = Vec::new();
    settings_page_ui(ui, &app.settings, &mut events);
    actions.extend(events.into_iter().map(Action::from));
}
