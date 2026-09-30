//! A job's twenty-nine steps as the nine stages the window shows, each with where it stands.
//!
//! **Role:** turn the step states of a running job, or the steps a failed job had finished, into
//! one row per stage: kept from an earlier run, still to run, running (its share done and seconds
//! so far), done (in its seconds) or failed, each with its steps' own lines; the shot scan, which
//! runs in the background until the cues join it, shows as such on its line.
//!
//! **Position:** called by the queue's stage list for the selected job; reads
//! `job_queue::models` and `core::steps`.
//!
//! **Signals and state:** none; pure.
//!
//! **Invariants:** always nine rows in run order, whose steps read in order are `StepName::ALL`; a
//! step this run does not do is kept, never to run; a failed step makes its stage failed, and a
//! running one its stage running, but for the shot scan, which never holds its stage open; a
//! failed job's list keeps exactly the steps its failure counts as kept.

use std::time::Instant;

use job_model::StepName;

use crate::core::steps::{STAGES, Stage};
use crate::job_queue::models::progress::{FinishedStep, JobProgress, StepState};
use crate::job_queue::models::queue::Failure;

/// Where one step or one stage stands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum StageState {
    /// Valid from an earlier run.
    Kept,
    /// Still to run.
    Pending,
    /// Under way: the share done (0 to 1) and the seconds so far.
    Running {
        share: f32,
        seconds: f64,
    },
    /// A step running in the background, which its stage does not wait for: the shot scan.
    Background,
    /// Done in this run, in `seconds` when known.
    Done {
        seconds: Option<f64>,
    },
    Failed,
}

/// One step's line under its stage.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StepLine {
    pub(crate) step: StepName,
    pub(crate) state: StageState,
}

/// One stage's row, with its steps' lines.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StageRow {
    pub(crate) stage: &'static Stage,
    pub(crate) state: StageState,
    pub(crate) steps: Vec<StepLine>,
}

/// The stage rows of a running job at `now`.
pub(crate) fn running(progress: &JobProgress, now: Instant) -> Vec<StageRow> {
    rows(|step| {
        let Some(row) = progress.steps.iter().find(|row| row.step == step) else {
            return StageState::Pending;
        };
        match &row.state {
            StepState::Pending if row.stale => StageState::Pending,
            StepState::Pending | StepState::Skipped => StageState::Kept,
            StepState::Running { .. } if step == StepName::ShotScan => StageState::Background,
            StepState::Running {
                started,
                done,
                total,
                ..
            } => StageState::Running {
                share: if *total > 0 {
                    (*done as f32 / *total as f32).min(1.0)
                } else {
                    0.0
                },
                seconds: now.saturating_duration_since(*started).as_secs_f64(),
            },
            StepState::Done { wall_s } => StageState::Done {
                seconds: Some(*wall_s),
            },
            StepState::Failed(_) => StageState::Failed,
        }
    })
}

/// The stage rows of a job that failed at `failure.step`: the steps it had finished done (with
/// their seconds) or kept as still valid, the failed step failed, every other step to run.
pub(crate) fn failed(failure: &Failure) -> Vec<StageRow> {
    rows(|step| {
        if failure.step == Some(step) {
            return StageState::Failed;
        }
        match failure
            .finished
            .iter()
            .find(|(finished, _)| *finished == step)
        {
            Some((_, FinishedStep::Done(seconds))) => StageState::Done { seconds: *seconds },
            Some((_, FinishedStep::StillValid)) => StageState::Kept,
            None => StageState::Pending,
        }
    })
}

/// The number of `step` among all steps, from 1: "step 9 of 29".
pub(crate) fn step_number(step: StepName) -> usize {
    StepName::ALL
        .iter()
        .position(|&s| s == step)
        .map_or(StepName::ALL.len(), |index| index + 1)
}

/// One row per stage, its steps' states from `state_of`.
fn rows(state_of: impl Fn(StepName) -> StageState) -> Vec<StageRow> {
    STAGES
        .iter()
        .map(|stage| {
            let steps: Vec<StepLine> = stage
                .steps
                .iter()
                .map(|&step| StepLine {
                    step,
                    state: state_of(step),
                })
                .collect();
            StageRow {
                stage,
                state: stage_state(&steps),
                steps,
            }
        })
        .collect()
}

/// A stage's state from its steps', leaving out a shot scan still to run or running in the
/// background: failed when one failed, running when one runs or some are done while others wait,
/// kept when all are kept, done when none waits (in the sum of their seconds, when every one is
/// known), else to run.
fn stage_state(steps: &[StepLine]) -> StageState {
    let states: Vec<StageState> = steps
        .iter()
        .filter(|line| {
            line.step != StepName::ShotScan
                || !matches!(line.state, StageState::Pending | StageState::Background)
        })
        .map(|line| line.state)
        .collect();
    if states.contains(&StageState::Failed) {
        return StageState::Failed;
    }
    let (mut shares, mut seconds, mut known) = (0.0_f32, 0.0_f64, true);
    let (mut running, mut done, mut pending) = (false, false, false);
    for state in &states {
        match *state {
            StageState::Kept => shares += 1.0,
            StageState::Done { seconds: s } => {
                shares += 1.0;
                match s {
                    Some(s) => seconds += s,
                    None => known = false,
                }
                done = true;
            }
            StageState::Running { share, seconds: s } => {
                shares += share;
                seconds += s;
                running = true;
            }
            StageState::Pending => pending = true,
            StageState::Background | StageState::Failed => {}
        }
    }
    if running || (done && pending) {
        StageState::Running {
            share: shares / states.len().max(1) as f32,
            seconds,
        }
    } else if pending {
        StageState::Pending
    } else if done {
        StageState::Done {
            seconds: known.then_some(seconds),
        }
    } else {
        StageState::Kept
    }
}

#[cfg(test)]
#[path = "tests/stage_progress.rs"]
mod tests;
