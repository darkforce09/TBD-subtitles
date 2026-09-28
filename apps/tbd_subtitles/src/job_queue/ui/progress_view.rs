//! The selected job on the right: its state, its time, and a row per step.
//!
//! **Role:** draw the running job's stage, clock, time left and share done, with each step's
//! state (to run, still valid, running with its progress, done with its time, failed with its
//! reason), or where a job that is not running stands: its place in line, the stage and step it
//! failed at, or the finished steps it kept.
//!
//! **Position:** called by the application's Jobs page for the selected job.
//!
//! **Signals and state:** none; reads the borrowed view and returns events.
//!
//! **Invariants:** a step this run does not do shows as still valid, never as to run.

use eframe::egui::{Grid, ProgressBar, RichText, Ui};

use crate::core::format;
use crate::core::steps::{stage_of, step_title};
use crate::core::ui::palette::palette;
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::progress::{JobProgress, StepState};
use crate::job_queue::models::queue::{Failure, JobState, QueueItem};
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
    ui.label(RichText::new(item.video.display().to_string()).color(palette(ui).text2));
    ui.add_space(6.0);
    match &item.state {
        JobState::Waiting => {
            let place = view
                .queue
                .items
                .iter()
                .filter(|other| other.kind == item.kind && other.state.is_waiting())
                .position(|other| other.id == item.id)
                .map_or(1, |at| at + 1);
            ui.label(format!(
                "Waiting to run · {} in line.",
                format::ordinal(place)
            ));
        }
        JobState::Running(progress) => running_ui(ui, view, item, progress, events),
        JobState::Failed(failure) => failed_ui(ui, failure),
        JobState::Cancelled { kept_steps } => {
            ui.label(format!(
                "Cancelled. {} kept; Retry resumes after them.",
                format::plural(*kept_steps, "finished step")
            ));
        }
        JobState::Finished(_) | JobState::FinishedBefore => {
            ui.label("Finished.");
        }
    }
}

fn failed_ui(ui: &mut Ui, failure: &Failure) {
    let (heading, detail) = match failure.step {
        Some(step) => (
            format!("Failed at {}.", stage_of(step).title),
            format!("{}: {}", step_title(step), failure.message),
        ),
        None => ("Failed.".to_string(), failure.message.clone()),
    };
    ui.label(RichText::new(heading).color(palette(ui).bad).strong());
    ui.label(RichText::new(detail).color(palette(ui).bad));
    ui.label(format!(
        "{} kept; Retry resumes after them.",
        format::plural(failure.kept_steps, "finished step")
    ));
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
    if let Some(step) = progress.current_step() {
        ui.label(RichText::new(format!("{}…", stage_of(step).doing)).strong());
    }
    ui.horizontal(|ui| {
        ui.add(
            ProgressBar::new(estimate.map_or(0.0, |(_, s)| s as f32))
                .desired_width(260.0)
                .fill(palette(ui).accent_fill),
        );
        let left = estimate.map_or_else(
            || "time left: once the video is probed".to_string(),
            |(left, _)| format!("{} left", format::about(left)),
        );
        ui.label(format!("{} running · {left}", format::duration(elapsed)));
        if progress.cancelling {
            ui.label(RichText::new("stopping…").color(palette(ui).warn));
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
                ui.label(step_title(row.step));
                match &row.state {
                    StepState::Pending if row.stale => {
                        ui.label(RichText::new("to run").color(palette(ui).text2));
                        ui.label("");
                    }
                    StepState::Pending | StepState::Skipped => {
                        ui.label(RichText::new("still valid").color(palette(ui).text2));
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
                        ui.horizontal(|ui| {
                            ui.add(
                                ProgressBar::new(share)
                                    .desired_width(160.0)
                                    .fill(palette(ui).accent_fill),
                            );
                            ui.label(
                                RichText::new(format!("{} %", (share * 100.0) as u32))
                                    .color(palette(ui).text2),
                            );
                        });
                        let running = view.now.saturating_duration_since(*started).as_secs_f64();
                        let text = match message {
                            Some(m) => format!("{} · {m}", format::duration(running)),
                            None => format::duration(running),
                        };
                        ui.label(RichText::new(text).color(palette(ui).text2));
                    }
                    StepState::Done { wall_s } => {
                        ui.label(RichText::new("✓ done").color(palette(ui).good));
                        ui.label(format::duration(*wall_s));
                    }
                    StepState::Failed(message) => {
                        ui.label(RichText::new("✗ failed").color(palette(ui).bad));
                        ui.label(RichText::new(message).color(palette(ui).bad));
                    }
                }
                ui.end_row();
            }
        });
}
