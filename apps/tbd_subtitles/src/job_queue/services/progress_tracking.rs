//! A running job's events folded into its progress: which steps it does, which are done, the
//! one running and how far it is.

use std::time::Instant;

use pipeline::progress::Progress;

use crate::job_queue::models::progress::{JobProgress, StepState};

/// Apply one event from the job runner, received at `now`.
pub(crate) fn apply(progress: &mut JobProgress, event: Progress, now: Instant) {
    match event {
        Progress::JobStarted {
            work_dir, stale, ..
        } => {
            progress.work_dir = Some(work_dir);
            for row in &mut progress.steps {
                row.stale = stale.contains(&row.step);
            }
        }
        Progress::JobDuration(seconds) => progress.duration_s = Some(seconds),
        Progress::StepSkipped(step) => set(progress, step, StepState::Skipped),
        Progress::StepStarted(step) => set(
            progress,
            step,
            StepState::Running {
                started: now,
                done: 0,
                total: 0,
                message: None,
            },
        ),
        Progress::StepAdvanced { step, done, total } => {
            if let Some(row) = progress.row_mut(step)
                && let StepState::Running {
                    done: d, total: t, ..
                } = &mut row.state
            {
                (*d, *t) = (done, total);
            }
        }
        Progress::StepMessage { step, text } => {
            if let Some(row) = progress.row_mut(step)
                && let StepState::Running { message, .. } = &mut row.state
            {
                *message = Some(text);
            }
        }
        Progress::StepFinished { step, measure } => set(
            progress,
            step,
            StepState::Done {
                wall_s: measure.wall_s,
            },
        ),
        Progress::StepFailed { step, message } => set(progress, step, StepState::Failed(message)),
        Progress::ModelCall { .. } => {}
    }
}

fn set(progress: &mut JobProgress, step: job_model::StepName, state: StepState) {
    if let Some(row) = progress.row_mut(step) {
        row.state = state;
    }
}

#[cfg(test)]
#[path = "tests/progress_tracking.rs"]
mod tests;
