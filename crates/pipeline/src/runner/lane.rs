//! The visual lane: text detection, reading and tracking on a thread of their own beside the
//! main walk, from the first step that waits on Claude until the first step that reads them.
//!
//! **Role:** decide, at each step of the main walk, whether to wait for the shot scan, start the
//! lane, wait for the lane, and run the step there; walk the lane's steps in order on its thread.
//!
//! **Position:** used by `runner` (`run_job`); runs each lane step through `runner::walk::Steps`,
//! as the main walk does; reads `graph::VISUAL_LANE` and its start point.
//!
//! **Signals and state:** one scoped thread per run that starts the lane; each lane step runs in
//! its own `step` span, made on the runner's thread as a child of the job's span, so its lines,
//! its worker's and its programs' group under its job; a lane step's failure stops the job
//! through `Steps`.
//!
//! **Invariants:** the lane starts after the shot scan has finished, since every lane step reads
//! it; the main walk runs no lane step and waits for the lane before any step that reads one; the
//! lane's thread stays alive until its worker is reaped; GPU steps of the lane and the main walk
//! still take the GPU lock one at a time.

use std::thread::{Scope, ScopedJoinHandle};

use job_model::StepName;
use tracing::Span;

use crate::error::{PipelineError, Result};
use crate::graph::{self, VISUAL_LANE, VISUAL_LANE_STARTS_AT};

use super::walk::{Steps, Tally};

/// Where the visual lane stands on the main walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LaneState {
    /// Not started: the main walk has not reached its start point.
    Waiting,
    /// Running on its thread.
    Running,
    /// Finished and joined.
    Joined,
}

/// What the main walk does at one step, in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Plan {
    /// Wait for the shot scan, when it still runs.
    pub(super) join_shots: bool,
    /// Start the visual lane.
    pub(super) spawn_lane: bool,
    /// Wait for the visual lane.
    pub(super) join_lane: bool,
    /// Run the step on the main walk; a lane step runs on the lane.
    pub(super) run_here: bool,
}

/// What the main walk does at `step` with the lane at `lane`.
pub(super) fn plan(step: StepName, lane: LaneState) -> Plan {
    let spawn_lane = lane == LaneState::Waiting && step == VISUAL_LANE_STARTS_AT;
    let in_lane = graph::in_visual_lane(step);
    Plan {
        join_shots: spawn_lane || graph::inputs(step).contains(&StepName::ShotScan),
        spawn_lane,
        join_lane: lane == LaneState::Running && !in_lane && graph::reads_visual_lane(step),
        run_here: !in_lane,
    }
}

/// Start the lane on a thread of `scope`; each step's span is a child of `job`.
pub(super) fn spawn<'scope>(
    scope: &'scope Scope<'scope, '_>,
    steps: &'scope Steps<'scope>,
    job: &Span,
) -> ScopedJoinHandle<'scope, Result<Tally>> {
    let spans: Vec<(StepName, Span)> = VISUAL_LANE
        .iter()
        .map(|&step| (step, tracing::info_span!(parent: job, "step", step = %step)))
        .collect();
    scope.spawn(move || {
        let walked = walk(steps, spans);
        if let Err(error) = &walked {
            steps.note_failure(error);
        }
        walked
    })
}

/// Wait for the lane's thread; a thread that panicked is a failed lane.
pub(super) fn join(handle: ScopedJoinHandle<'_, Result<Tally>>) -> Result<Tally> {
    handle
        .join()
        .map_err(|_| PipelineError::new("the visual lane", "the lane's thread panicked"))?
}

/// Run the lane's steps in order, each in its span, until one fails or the job stops.
fn walk(steps: &Steps, spans: Vec<(StepName, Span)>) -> Result<Tally> {
    let mut tally = Tally::default();
    for (step, span) in spans {
        let _in_step = span.enter();
        steps.run(step, &mut tally)?;
    }
    Ok(tally)
}

#[cfg(test)]
#[path = "tests/lane.rs"]
mod tests;
