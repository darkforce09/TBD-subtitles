//! The correction runs' lanes: four job runners, each running one correction run at a time.
//!
//! **Role:** own [`REVIEW_LANES`] long-lived runner threads for correction runs, hand a run to an
//! idle one, remember which run each lane holds with the token that stops it, and gather what
//! every lane sends back.
//!
//! **Position:** started by the application when the window opens, beside the full run's runner;
//! handed runs and emptied by the application's runner actions, which also find a run's token to
//! cancel it.
//!
//! **Signals and state:** each lane's runner (`job_runner`) and the run it holds; the runners'
//! events are drained into the caller's list.
//!
//! **Invariants:** a lane holds at most one run and a run sits in at most one lane; a run is
//! handed only to an idle lane, so no correction run ever waits behind another inside a runner;
//! a lane is free again only once the application releases its run.

use pipeline::CancelToken;

use crate::core::background::Wake;
use crate::job_queue::models::queue::JobId;
use crate::job_queue::services::job_runner::{self, Command, JobRunner, RunJob, RunnerEvent};

/// How many correction runs run at once. Each runs its one stale model step on the CPU, in about
/// 3 to 5 s and 2.4 GB of RAM.
pub(crate) const REVIEW_LANES: usize = 4;

/// The correction runs' runners and what each holds.
pub(crate) struct ReviewLanes {
    lanes: Vec<Lane>,
}

/// One runner, and the run it holds with the token that stops it.
struct Lane {
    runner: JobRunner,
    running: Option<(JobId, CancelToken)>,
}

impl Lane {
    fn idle(&self) -> bool {
        self.running.is_none()
    }
}

impl ReviewLanes {
    /// Start [`REVIEW_LANES`] runner threads, "review-runner-1" onwards, each running jobs with
    /// `run` and waking the window with `wake`.
    pub(crate) fn start(run: RunJob, wake: Wake) -> ReviewLanes {
        let lanes = (1..=REVIEW_LANES)
            .map(|n| Lane {
                runner: job_runner::start(&format!("review-runner-{n}"), run.clone(), wake.clone()),
                running: None,
            })
            .collect();
        ReviewLanes { lanes }
    }

    /// Whether a lane is idle.
    pub(crate) fn free(&self) -> bool {
        self.lanes.iter().any(Lane::idle)
    }

    /// Hand run `id` to the first idle lane, which holds it with `token`; an error when no lane
    /// is idle or the lane's runner has stopped, and then no lane holds it.
    pub(crate) fn run(
        &mut self,
        id: JobId,
        command: Command,
        token: CancelToken,
    ) -> Result<(), String> {
        let Some(lane) = self.lanes.iter().find(|lane| lane.idle()) else {
            return Err("every correction run lane is busy".to_string());
        };
        lane.runner.run(command)?;
        // The same lane: the first idle one.
        self.hold(id, token);
        Ok(())
    }

    /// Mark the first idle lane as holding run `id` without handing its runner anything; whether
    /// a lane was idle.
    pub(crate) fn hold(&mut self, id: JobId, token: CancelToken) -> bool {
        match self.lanes.iter_mut().find(|lane| lane.idle()) {
            Some(lane) => {
                lane.running = Some((id, token));
                true
            }
            None => false,
        }
    }

    /// Free the lane that holds run `id`, when one does.
    pub(crate) fn release(&mut self, id: JobId) {
        for lane in &mut self.lanes {
            if lane.running.as_ref().is_some_and(|(held, _)| *held == id) {
                lane.running = None;
            }
        }
    }

    /// Whether a lane holds run `id`.
    #[cfg(test)]
    pub(crate) fn is_running(&self, id: JobId) -> bool {
        self.token(id).is_some()
    }

    /// The token that stops run `id`, while a lane holds it.
    pub(crate) fn token(&self, id: JobId) -> Option<&CancelToken> {
        self.held()
            .find(|(held, _)| *held == id)
            .map(|(_, token)| token)
    }

    /// The tokens of every run the lanes hold.
    #[cfg(test)]
    pub(crate) fn tokens(&self) -> impl Iterator<Item = &CancelToken> {
        self.held().map(|(_, token)| token)
    }

    /// Move every event the lanes' runners sent so far onto `into`, lane by lane.
    pub(crate) fn drain(&self, into: &mut Vec<RunnerEvent>) {
        for lane in &self.lanes {
            while let Ok(event) = lane.runner.events.try_recv() {
                into.push(event);
            }
        }
    }

    /// Each run a lane holds, with its token.
    fn held(&self) -> impl Iterator<Item = &(JobId, CancelToken)> {
        self.lanes.iter().filter_map(|lane| lane.running.as_ref())
    }
}

#[cfg(test)]
#[path = "tests/review_lanes.rs"]
mod tests;
