//! The queue panel: the add buttons, Start and Pause, and one row per job with its state, its
//! progress and time left, and the buttons that move, cancel, retry or remove it.
//!
//! **Role:** draw the borrowed queue and turn each click into a `JobQueueEvent`.
//!
//! **Position:** called by the application's frame for the left panel.
//!
//! **Signals and state:** none; reads the view and returns events.
//!
//! **Invariants:** a running job offers only Cancel; only waiting jobs move.

use eframe::egui::{Button, ProgressBar, RichText, ScrollArea, Ui};

use crate::core::format;
use crate::core::ui::{BAD, CAUTION, GOOD, MUTED_TEXT};
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::queue::{JobState, Move, QueueItem};
use crate::job_queue::models::view::JobQueueView;
use crate::job_queue::services::time_left;

/// Draw the queue from `view` and push what the user asked for onto `events`.
pub(crate) fn queue_panel_ui(
    ui: &mut Ui,
    view: &JobQueueView<'_>,
    events: &mut Vec<JobQueueEvent>,
) {
    let queue = view.queue;
    ui.heading("Queue");
    ui.horizontal(|ui| {
        if ui.button("Add videos…").clicked() {
            events.push(JobQueueEvent::AddVideos);
        }
        if ui.button("Add folder…").clicked() {
            events.push(JobQueueEvent::AddFolder);
        }
    });
    ui.horizontal(|ui| {
        if queue.running {
            if ui
                .button("⏸ Pause")
                .on_hover_text("Start no further job")
                .clicked()
            {
                events.push(JobQueueEvent::Pause);
            }
        } else {
            let can_start = !view.models_missing && queue.count(JobState::is_waiting) > 0;
            if ui
                .add_enabled(can_start, Button::new("▶ Start"))
                .on_hover_text("Run the waiting jobs, one after another")
                .clicked()
            {
                events.push(JobQueueEvent::Start);
            }
        }
        ui.label(
            RichText::new(format!(
                "{} waiting · {} done",
                queue.count(JobState::is_waiting),
                queue.count(|s| matches!(s, JobState::Finished(_) | JobState::FinishedBefore))
            ))
            .color(MUTED_TEXT),
        );
    });
    if view.models_missing {
        ui.label(RichText::new("Models are missing: download them in Settings.").color(BAD));
    }
    ui.separator();
    if queue.items.is_empty() {
        ui.label(
            RichText::new("No videos queued. Add videos or a folder, or drop them here.")
                .color(MUTED_TEXT),
        );
        return;
    }
    ScrollArea::vertical().show(ui, |ui| {
        for item in &queue.items {
            row_ui(ui, view, item, events);
            ui.separator();
        }
    });
}

fn row_ui(ui: &mut Ui, view: &JobQueueView<'_>, item: &QueueItem, events: &mut Vec<JobQueueEvent>) {
    let name = item.video.file_name().map_or_else(
        || item.video.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let (mark, colour) = match &item.state {
        JobState::Waiting => ("…", MUTED_TEXT),
        JobState::Running(_) => ("▶", CAUTION),
        JobState::Finished(result) if result.failures.is_empty() => ("✓", GOOD),
        JobState::Finished(_) => ("⚠", CAUTION),
        JobState::FinishedBefore => ("✓", GOOD),
        JobState::Failed(_) => ("✗", BAD),
        JobState::Cancelled => ("■", MUTED_TEXT),
    };
    let selected = view.queue.selected == Some(item.id);
    ui.horizontal(|ui| {
        ui.label(RichText::new(mark).color(colour));
        if ui
            .selectable_label(selected, name)
            .on_hover_text(item.video.display().to_string())
            .clicked()
        {
            events.push(JobQueueEvent::Select(item.id));
        }
    });
    ui.horizontal(|ui| match &item.state {
        JobState::Running(progress) => {
            let estimate = time_left::estimate(progress, view.rates, view.now);
            let share = estimate.map_or(0.0, |(_, share)| share as f32);
            ui.add(ProgressBar::new(share).desired_width(150.0));
            let text = match (progress.cancelling, estimate) {
                (true, _) => "stopping…".to_string(),
                (false, Some((left, _))) => format!("{} left", format::duration(left)),
                (false, None) => "starting…".to_string(),
            };
            ui.label(RichText::new(text).color(MUTED_TEXT));
            if !progress.cancelling && ui.small_button("Cancel").clicked() {
                events.push(JobQueueEvent::Cancel(item.id));
            }
        }
        JobState::Waiting => {
            for (label, to, tip) in [
                ("Up", Move::Up, "Run it earlier"),
                ("Down", Move::Down, "Run it later"),
                ("Next", Move::Top, "Run it next"),
            ] {
                if ui.small_button(label).on_hover_text(tip).clicked() {
                    events.push(JobQueueEvent::Move(item.id, to));
                }
            }
            if ui.small_button("✕").on_hover_text("Remove").clicked() {
                events.push(JobQueueEvent::Remove(item.id));
            }
        }
        _ => {
            if ui.small_button("↻ Retry").clicked() {
                events.push(JobQueueEvent::Retry(item.id));
            }
            if ui.small_button("✕").on_hover_text("Remove").clicked() {
                events.push(JobQueueEvent::Remove(item.id));
            }
        }
    });
}
