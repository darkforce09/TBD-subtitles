use std::time::Instant;

use super::*;

fn set(progress: &mut JobProgress, step: StepName, state: StepState) {
    if let Some(row) = progress.row_mut(step) {
        row.state = state;
    }
}

fn running() -> StepState {
    StepState::Running {
        started: Instant::now(),
        done: 0,
        total: 0,
        message: None,
    }
}

#[test]
fn done_and_still_valid_steps_are_kept_and_the_later_running_step_is_current() {
    let mut p = JobProgress::new(Instant::now());
    assert_eq!(p.finished_steps().len(), 0);
    assert_eq!(p.current_step(), None);
    set(
        &mut p,
        StepName::ProbeDecode,
        StepState::Done { wall_s: 4.0 },
    );
    set(&mut p, StepName::ShotScan, running());
    set(&mut p, StepName::Separation, StepState::Skipped);
    set(&mut p, StepName::Vad, running());
    if let Some(row) = p.row_mut(StepName::Output) {
        row.stale = false;
    }
    assert_eq!(
        p.finished_steps().len(),
        3,
        "done, skipped, and a step this run does not do"
    );
    assert_eq!(
        p.current_step(),
        Some(StepName::Vad),
        "beside the shot scan"
    );
    assert_eq!(p.failed_step(), None);
}

#[test]
fn a_failed_step_is_named_and_not_kept() {
    let mut p = JobProgress::new(Instant::now());
    set(&mut p, StepName::ProbeDecode, StepState::Skipped);
    set(
        &mut p,
        StepName::AsrWhisper,
        StepState::Failed("boom".into()),
    );
    assert_eq!(p.failed_step(), Some(StepName::AsrWhisper));
    assert_eq!(p.finished_steps().len(), 1);
    assert_eq!(p.current_step(), None);
}

#[test]
fn the_step_shown_is_the_last_started_and_never_the_shot_scan() {
    let mut p = JobProgress::new(Instant::now());
    assert_eq!(p.shown_step(), None, "nothing started yet");
    set(&mut p, StepName::ShotScan, running());
    assert_eq!(p.shown_step(), None);
    set(
        &mut p,
        StepName::ProbeDecode,
        StepState::Done { wall_s: 4.0 },
    );
    set(
        &mut p,
        StepName::Separation,
        StepState::Done { wall_s: 90.0 },
    );
    assert_eq!(
        p.current_step(),
        Some(StepName::ShotScan),
        "between two steps only the shot scan runs"
    );
    assert_eq!(p.shown_step(), Some(StepName::Separation));
    set(&mut p, StepName::Vad, running());
    assert_eq!(p.shown_step(), Some(StepName::Vad));
    assert_eq!(
        p.finished_steps(),
        [
            (StepName::ProbeDecode, FinishedStep::Done(Some(4.0))),
            (StepName::Separation, FinishedStep::Done(Some(90.0))),
        ],
        "the shot scan still running is not finished"
    );
}

#[test]
fn a_lane_step_beside_a_main_step_runs_in_the_background() {
    let mut p = JobProgress::new(Instant::now());
    set(&mut p, StepName::TextDetect, running());
    assert!(!p.runs_in_background(StepName::TextDetect), "it runs alone");
    assert_eq!(p.shown_step(), Some(StepName::TextDetect));
    set(&mut p, StepName::Adjudicate, running());
    assert!(p.runs_in_background(StepName::TextDetect));
    assert!(p.runs_in_background(StepName::ShotScan));
    assert!(!p.runs_in_background(StepName::Adjudicate));
    assert_eq!(p.shown_step(), Some(StepName::Adjudicate));
    assert_eq!(p.current_step(), Some(StepName::Adjudicate));
}

#[test]
fn a_lane_step_done_before_the_join_is_not_named_between_main_steps() {
    let mut p = JobProgress::new(Instant::now());
    set(
        &mut p,
        StepName::Adjudicate,
        StepState::Done { wall_s: 120.0 },
    );
    set(
        &mut p,
        StepName::TextDetect,
        StepState::Done { wall_s: 200.0 },
    );
    assert_eq!(p.shown_step(), Some(StepName::Adjudicate));
    set(&mut p, StepName::TextRead, running());
    assert_eq!(
        p.shown_step(),
        Some(StepName::TextRead),
        "only the lane runs"
    );
    set(&mut p, StepName::TextRead, StepState::Done { wall_s: 30.0 });
    set(&mut p, StepName::TextTranslate, running());
    assert_eq!(p.shown_step(), Some(StepName::TextTranslate));
}
