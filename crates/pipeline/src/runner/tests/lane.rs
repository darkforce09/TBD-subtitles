use std::sync::Mutex;

use job_model::job::{JobRecord, StepMeasure};

use super::*;
use crate::cancel::CancelToken;
use crate::progress::Progress;
use crate::work_dir::store::scratch::{Scratch, job_record};
use crate::workers::StepWrite;

fn position(step: StepName) -> usize {
    StepName::ALL
        .iter()
        .position(|s| *s == step)
        .expect("listed")
}

/// The main walk's plans over every step, with the lane's state moved as the runner moves it.
fn walk_plans() -> Vec<(StepName, Plan)> {
    let mut lane = LaneState::Waiting;
    StepName::ALL
        .into_iter()
        .map(|step| {
            let plan = plan(step, lane);
            if plan.spawn_lane {
                lane = LaneState::Running;
            }
            if plan.join_lane {
                lane = LaneState::Joined;
            }
            (step, plan)
        })
        .collect()
}

#[test]
fn the_lane_starts_at_adjudication_and_is_joined_before_translation() {
    let plans = walk_plans();
    let spawned: Vec<StepName> = plans
        .iter()
        .filter(|(_, p)| p.spawn_lane)
        .map(|(s, _)| *s)
        .collect();
    let joined: Vec<StepName> = plans
        .iter()
        .filter(|(_, p)| p.join_lane)
        .map(|(s, _)| *s)
        .collect();
    assert_eq!(spawned, vec![StepName::Adjudicate]);
    assert_eq!(joined, vec![StepName::TextTranslate]);
}

#[test]
fn the_main_walk_runs_every_step_but_the_lanes() {
    for (step, plan) in walk_plans() {
        assert_eq!(plan.run_here, !VISUAL_LANE.contains(&step), "{step}");
    }
}

#[test]
fn the_shot_scan_is_joined_before_the_lane_starts_and_before_its_readers() {
    for (step, plan) in walk_plans() {
        if plan.spawn_lane || graph::inputs(step).contains(&StepName::ShotScan) {
            assert!(plan.join_shots, "{step}");
        }
    }
}

#[test]
fn every_main_step_that_reads_the_lane_comes_after_its_join() {
    let join = walk_plans()
        .into_iter()
        .find(|(_, p)| p.join_lane)
        .map(|(s, _)| position(s))
        .expect("joined");
    for step in StepName::ALL {
        if !graph::in_visual_lane(step) && graph::reads_visual_lane(step) {
            assert!(
                join <= position(step),
                "{step} reads the lane before its join"
            );
        }
    }
}

/// Runs the lane on a scratch job whose steps end as `outcome` says; its result and events.
fn run_lane(
    outcome: impl Fn(StepName) -> Result<()> + Sync,
) -> (Result<Tally>, Vec<Progress>, bool) {
    let scratch = Scratch::new("lane");
    let record = job_record(&scratch.dir);
    let events = Mutex::new(Vec::new());
    let sink = |event| events.lock().expect("events").push(event);
    let cancel = CancelToken::new();
    let measured = |step: StepName, _: &JobRecord| {
        outcome(step).map(|()| (StepMeasure::default(), None::<StepWrite>))
    };
    let steps = Steps::new(scratch.store(), &record, None, &measured, &sink, &cancel);
    let result = std::thread::scope(|scope| join(spawn(scope, &steps, &Span::none())));
    drop(steps);
    (
        result,
        events.into_inner().expect("events"),
        cancel.is_cancelled(),
    )
}

#[test]
fn the_lane_runs_its_steps_in_order_on_its_thread() {
    let (result, events, cancelled) = run_lane(|_| Ok(()));
    let tally = result.expect("the lane finishes");
    assert_eq!(tally.ran, VISUAL_LANE.to_vec());
    let finished: Vec<StepName> = events
        .iter()
        .filter_map(|event| match event {
            Progress::StepFinished { step, .. } => Some(*step),
            _ => None,
        })
        .collect();
    assert_eq!(finished, VISUAL_LANE.to_vec());
    assert!(!cancelled);
}

#[test]
fn a_failed_lane_step_stops_the_lane_and_the_job() {
    let (result, events, cancelled) = run_lane(|step| {
        if step == StepName::TextRead {
            Err(PipelineError::new("step text_read", "the worker exited"))
        } else {
            Ok(())
        }
    });
    assert_eq!(
        result,
        Err(PipelineError::new("step text_read", "the worker exited"))
    );
    assert!(cancelled);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Progress::StepStarted(StepName::TextTrack)))
    );
    let failed: Vec<StepName> = events
        .iter()
        .filter_map(|event| match event {
            Progress::StepFailed { step, .. } => Some(*step),
            _ => None,
        })
        .collect();
    assert_eq!(failed, vec![StepName::TextRead]);
}
