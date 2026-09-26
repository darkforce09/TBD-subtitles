//! The selected job on the right: its state, its time, and a row per step.
//!
//! **Role:** draw the running job's clock, time left and share done, with each step's state (to
//! run, still valid, running with its progress, done with its time, failed with its reason), or
//! where a job that is not running stands.
//!
//! **Position:** called by the application's Jobs page for the selected job.
//!
//! **Signals and state:** none; reads the borrowed view and returns events.
//!
//! **Invariants:** a step this run does not do shows as still valid, never as to run.

use eframe::egui::{Grid, ProgressBar, RichText, Ui};

use crate::core::format;
use crate::core::ui::{BAD, CAUTION, GOOD, MUTED_TEXT};
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::progress::{JobProgress, StepState};
use crate::job_queue::models::queue::{JobState, QueueItem};
use crate::job_queue::models::view::JobQueueView;
use crate::job_queue::services::time_left;

/// Draw `item`'s progress, or where it stands when it is not running.
pub(crate) fn progress_view_ui(
    ui: &mut Ui,
    view: &JobQueueView<'_>,
    item: &QueueItem,
    events: &mut Vec<JobQueueEvent>,
) {
    let name = item.video.file_name().map_or_else(
        || item.video.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    ui.heading(name);
    ui.label(RichText::new(item.video.display().to_string()).color(MUTED_TEXT));
    ui.add_space(6.0);
    match &item.state {
        JobState::Waiting => {
            ui.label("Waiting to run.");
        }
        JobState::Running(progress) => running_ui(ui, view, item, progress, events),
        JobState::Failed(reason) => {
            ui.label(RichText::new(format!("Failed: {reason}")).color(BAD));
        }
        JobState::Cancelled => {
            ui.label("Cancelled. Its finished steps are kept; Retry resumes after them.");
        }
        JobState::Finished(_) | JobState::FinishedBefore => {
            ui.label("Finished.");
        }
    }
}

fn running_ui(
    ui: &mut Ui,
    view: &JobQueueView<'_>,
    item: &QueueItem,
    progress: &JobProgress,
    events: &mut Vec<JobQueueEvent>,
) {
    let elapsed = view
        .now
        .saturating_duration_since(progress.started)
        .as_secs_f64();
    let estimate = time_left::estimate(progress, view.rates, view.now);
    ui.horizontal(|ui| {
        ui.add(ProgressBar::new(estimate.map_or(0.0, |(_, s)| s as f32)).desired_width(260.0));
        let left = estimate.map_or_else(
            || "time left: once the video is probed".to_string(),
            |(left, _)| format!("about {} left", format::duration(left)),
        );
        ui.label(format!("{} running · {left}", format::duration(elapsed)));
        if progress.cancelling {
            ui.label(RichText::new("stopping…").color(CAUTION));
        } else if ui.button("Cancel").clicked() {
            events.push(JobQueueEvent::Cancel(item.id));
        }
    });
    ui.add_space(6.0);
    Grid::new("steps")
        .num_columns(3)
        .striped(true)
        .spacing([16.0, 4.0])
        .show(ui, |ui| {
            for row in &progress.steps {
                ui.label(row.step.as_str());
                match &row.state {
                    StepState::Pending if row.stale => {
                        ui.label(RichText::new("to run").color(MUTED_TEXT));
                        ui.label("");
                    }
                    StepState::Pending | StepState::Skipped => {
                        ui.label(RichText::new("still valid").color(MUTED_TEXT));
                        ui.label("");
                    }
                    StepState::Running {
                        started,
                        done,
                        total,
                        message,
                    } => {
                        let share = if *total > 0 {
                            *done as f32 / *total as f32
                        } else {
                            0.0
                        };
                        ui.add(
                            ProgressBar::new(share)
                                .desired_width(160.0)
                                .show_percentage(),
                        );
                        let running = view.now.saturating_duration_since(*started).as_secs_f64();
                        let text = match message {
                            Some(m) => format!("{} · {m}", format::duration(running)),
                            None => format::duration(running),
                        };
                        ui.label(RichText::new(text).color(MUTED_TEXT));
                    }
                    StepState::Done { wall_s } => {
                        ui.label(RichText::new("✓ done").color(GOOD));
                        ui.label(format::duration(*wall_s));
                    }
                    StepState::Failed(message) => {
                        ui.label(RichText::new("✗ failed").color(BAD));
                        ui.label(RichText::new(message).color(BAD));
                    }
                }
                ui.end_row();
            }
        });
}
