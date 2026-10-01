use std::sync::Mutex;

use super::*;
use crate::work_dir::store::scratch::{Scratch, job_record};

fn real(context: &str) -> PipelineError {
    PipelineError::new(context, "the worker exited with code 1")
}

#[test]
fn a_real_failure_is_always_reported_and_a_stop_only_when_none_came_first() {
    assert!(reports_failure(&real("step text_read"), false));
    assert!(reports_failure(&real("step text_read"), true));
    let stop = PipelineError::cancelled("step adjudicate");
    assert!(reports_failure(&stop, false));
    assert!(!reports_failure(&stop, true));
}

#[test]
fn a_walk_stopped_by_another_steps_failure_ends_with_that_failure() {
    let first = real("step text_read");
    let stop = PipelineError::cancelled("step adjudicate");
    assert_eq!(job_error(stop.clone(), Some(first.clone())), first);
    assert_eq!(job_error(stop.clone(), None), stop);
    let own = real("step alignment");
    assert_eq!(job_error(own.clone(), Some(first)), own);
}

#[test]
fn a_tally_merged_from_two_walks_lists_steps_in_run_order() {
    use StepName::*;
    let mut main = Tally {
        ran: vec![ProbeDecode, Adjudicate, TextTranslate],
        skipped: vec![ShotScan],
    };
    main.merge(Tally {
        ran: vec![TextDetect, TextTrack],
        skipped: vec![TextRead],
    });
    assert_eq!(
        main.in_run_order(),
        (
            vec![
                ProbeDecode,
                Adjudicate,
                TextDetect,
                TextTrack,
                TextTranslate
            ],
            vec![ShotScan, TextRead]
        )
    );
}

/// Runs `step` on a scratch job whose steps end as `outcome` says; the events it sent.
fn run_with(
    already_failed: Option<PipelineError>,
    step: StepName,
    outcome: impl Fn(StepName) -> Result<()> + Sync,
) -> (
    Result<()>,
    Vec<Progress>,
    CancelToken,
    Option<PipelineError>,
) {
    let scratch = Scratch::new("walk");
    let record = job_record(&scratch.dir);
    let events = Mutex::new(Vec::new());
    let sink = |event| events.lock().expect("events").push(event);
    let cancel = CancelToken::new();
    let measured = |step: StepName, _: &JobRecord| {
        outcome(step).map(|()| (StepMeasure::default(), None::<StepWrite>))
    };
    let steps = Steps::new(scratch.store(), &record, None, &measured, &sink, &cancel);
    if let Some(error) = already_failed {
        steps.failed(StepName::Adjudicate, &error);
    }
    let mut tally = Tally::default();
    let ran = steps.begin(step, &mut tally).and_then(|begun| match begun {
        Begun::Started { fingerprint } => steps.complete(step, fingerprint),
        Begun::Skipped => Ok(()),
    });
    let failure = steps.failure();
    drop(steps);
    (ran, events.into_inner().expect("events"), cancel, failure)
}

fn failed_steps(events: &[Progress]) -> Vec<StepName> {
    events
        .iter()
        .filter_map(|event| match event {
            Progress::StepFailed { step, .. } => Some(*step),
            _ => None,
        })
        .collect()
}

#[test]
fn a_finished_step_commits_its_record_and_reports_it() {
    let (ran, events, cancel, failure) = run_with(None, StepName::ProbeDecode, |_| Ok(()));
    assert!(ran.is_ok(), "{ran:?}");
    assert!(matches!(
        events[0],
        Progress::StepStarted(StepName::ProbeDecode)
    ));
    assert!(matches!(
        events[1],
        Progress::StepFinished {
            step: StepName::ProbeDecode,
            ..
        }
    ));
    assert!(!cancel.is_cancelled());
    assert!(failure.is_none());
}

#[test]
fn a_failed_step_reports_once_stops_the_job_and_is_its_failure() {
    let (ran, events, cancel, failure) =
        run_with(None, StepName::TextRead, |_| Err(real("step text_read")));
    assert_eq!(ran, Err(real("step text_read")));
    assert_eq!(failed_steps(&events), vec![StepName::TextRead]);
    assert!(cancel.is_cancelled());
    assert_eq!(failure, Some(real("step text_read")));
}

#[test]
fn a_step_stopped_by_another_steps_failure_reports_nothing() {
    let (ran, events, _, failure) = run_with(
        Some(real("step adjudicate")),
        StepName::TextDetect,
        |step| Err(PipelineError::cancelled(format!("step {step}"))),
    );
    assert!(ran.is_err_and(|error| error.is_cancelled()));
    assert_eq!(failed_steps(&events), vec![StepName::Adjudicate]);
    assert_eq!(failure, Some(real("step adjudicate")));
}

#[test]
fn a_step_the_owner_stopped_reports_the_stop() {
    let (ran, events, _, failure) = run_with(None, StepName::TextDetect, |step| {
        Err(PipelineError::cancelled(format!("step {step}")))
    });
    assert!(ran.is_err_and(|error| error.is_cancelled()));
    assert_eq!(failed_steps(&events), vec![StepName::TextDetect]);
    assert!(failure.is_none());
}
