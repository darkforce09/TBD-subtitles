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
    assert_eq!(p.kept_steps(), 0);
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
        p.kept_steps(),
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
    assert_eq!(p.kept_steps(), 1);
    assert_eq!(p.current_step(), None);
}
