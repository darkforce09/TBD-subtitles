//! The time a running job has left: each step's measured seconds per second of video, from the
//! jobs already in the work folder, applied to the steps still to run.
//!
//! **Role:** learn how long each step takes per second of video from earlier jobs' `job.json`
//! and `probe.json`, fall back to the Dressrosa 11 pilot's rates for a step never measured, and
//! estimate a running job's seconds left and its share done.
//!
//! **Position:** called by the application when the window opens and after each job; read by
//! the queue panel and the progress view.
//!
//! **Signals and state:** `from_history` reads the work folder; the rest is pure.
//!
//! **Invariants:** the shot scan, which runs alongside the other steps, never adds to the time
//! left; a running step that reports its progress is estimated from its own pace.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use job_model::StepName;
use job_model::job::JobRecord;
use job_model::outputs::ProbeDecoded;

use crate::job_queue::models::progress::{JobProgress, Rates, StepState};

/// Seconds per second of video on the Dressrosa 11 pilot (1853.7 s), for steps with no history.
const PILOT: &[(StepName, f64)] = &[
    (StepName::ProbeDecode, 4.8),
    (StepName::Separation, 103.6),
    (StepName::Vad, 0.6),
    (StepName::AsrParakeet, 8.7),
    (StepName::AsrWhisper, 74.7),
    (StepName::SoundEvents, 17.6),
    (StepName::Adjudicate, 52.9),
    (StepName::RedecodeParakeet, 2.9),
    (StepName::RedecodeWhisper, 3.5),
    (StepName::Readjudicate, 8.9),
    (StepName::SoundCues, 5.6),
    (StepName::Alignment, 7.8),
];
const PILOT_VIDEO_S: f64 = 1853.7;

/// The pilot's rates.
pub(crate) fn pilot_rates() -> Rates {
    Rates {
        per_step: PILOT
            .iter()
            .map(|&(step, wall)| (step, wall / PILOT_VIDEO_S))
            .collect(),
    }
}

/// The mean rate of each step over every job in `work_root` with a probe, over the pilot's.
pub(crate) fn from_history(work_root: &Path) -> Rates {
    let mut sums: BTreeMap<StepName, (f64, usize)> = BTreeMap::new();
    let jobs = std::fs::read_dir(work_root)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path());
    for job in jobs {
        let record = read::<JobRecord>(&job.join("job.json"));
        let probe = read::<ProbeDecoded>(&job.join("probe.json"));
        let (Some(record), Some(probe)) = (record, probe) else {
            continue;
        };
        if probe.probe.duration_s <= 0.0 {
            continue;
        }
        for (step, done) in &record.steps {
            let entry = sums.entry(*step).or_insert((0.0, 0));
            entry.0 += done.measure.wall_s / probe.probe.duration_s;
            entry.1 += 1;
        }
    }
    let mut rates = pilot_rates();
    for (step, (sum, n)) in sums {
        rates.per_step.insert(step, sum / n as f64);
    }
    rates
}

fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Seconds `step` is expected to take on `duration_s` of video.
fn expected(rates: &Rates, step: StepName, duration_s: f64) -> f64 {
    rates.per_step.get(&step).copied().unwrap_or(0.0) * duration_s
}

/// The seconds left, and the share of the job's expected time done; `None` until the video's
/// length is known.
pub(crate) fn estimate(progress: &JobProgress, rates: &Rates, now: Instant) -> Option<(f64, f64)> {
    let duration = progress.duration_s?;
    let (mut left, mut total) = (0.0, 0.0);
    for row in progress
        .steps
        .iter()
        .filter(|r| r.stale && r.step != StepName::ShotScan)
    {
        let expected = expected(rates, row.step, duration);
        total += expected;
        left += match &row.state {
            StepState::Pending => expected,
            StepState::Running {
                started,
                done,
                total: of,
                ..
            } => {
                let elapsed = now.saturating_duration_since(*started).as_secs_f64();
                if *of > 0 && *done > 0 {
                    // The step's own pace, once it reports one.
                    let share = (*done as f64 / *of as f64).min(1.0);
                    elapsed / share * (1.0 - share)
                } else {
                    (expected - elapsed).max(0.0)
                }
            }
            StepState::Skipped | StepState::Done { .. } | StepState::Failed(_) => 0.0,
        };
    }
    let share = if total > 0.0 {
        (1.0 - left / total).clamp(0.0, 1.0)
    } else {
        0.0
    };
    Some((left, share))
}

#[cfg(test)]
#[path = "tests/time_left.rs"]
mod tests;
