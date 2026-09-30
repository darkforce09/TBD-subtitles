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
//! left, and neither does a step the job's settings leave idle (the on-screen text steps with
//! translation off, the replacement steps and the localized video with it off), nor does such a
//! step's near-zero time lower the rate learnt from history; a running step that reports its
//! progress is estimated from its own pace.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use job_model::job::{JobRecord, JobSettings};
use job_model::outputs::ProbeDecoded;
use job_model::{StageName, StepName};

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
            if !works_in(*step, &record.settings) {
                continue;
            }
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

/// Whether a job with `settings` does work in `step`: the on-screen text steps only with
/// translation on, the replacement steps and the localized video only with the localized video
/// on too; every other step always.
pub(crate) fn works_in(step: StepName, settings: &JobSettings) -> bool {
    let text = &settings.onscreen_text;
    match step {
        StepName::TextMask
        | StepName::TextInpaint
        | StepName::TextCompose
        | StepName::TextVerify
        | StepName::LocalizedVideo => text.enabled && text.localized_video,
        step if step.stage() == StageName::OnscreenText => text.enabled,
        _ => true,
    }
}

/// The steps a job with `settings` runs with nothing to do, in run order.
pub(crate) fn idle_steps(settings: &JobSettings) -> Vec<StepName> {
    StepName::ALL
        .into_iter()
        .filter(|step| !works_in(*step, settings))
        .collect()
}

fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Seconds `step` is expected to take on `duration_s` of video.
fn expected(rates: &Rates, step: StepName, duration_s: f64) -> f64 {
    let initial = match step {
        StepName::TextDetect => 0.2,
        StepName::TextRead => 0.15,
        StepName::TextTrack => 0.25,
        StepName::TextTranslate => 0.2,
        StepName::TextReview | StepName::TextTypeset => 0.01,
        StepName::TextMask => 0.05,
        StepName::TextInpaint => 0.1,
        StepName::TextCompose => 0.02,
        StepName::TextVerify => 0.01,
        StepName::LocalizedVideo => 0.25,
        _ => 0.0,
    };
    rates.per_step.get(&step).copied().unwrap_or(initial) * duration_s
}

/// The seconds left, and the share of the job's expected time done; `None` until the video's
/// length is known.
pub(crate) fn estimate(progress: &JobProgress, rates: &Rates, now: Instant) -> Option<(f64, f64)> {
    let duration = progress.duration_s?;
    let (mut left, mut total) = (0.0, 0.0);
    for row in progress
        .steps
        .iter()
        .filter(|r| r.stale && r.step != StepName::ShotScan && !progress.idle.contains(&r.step))
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
