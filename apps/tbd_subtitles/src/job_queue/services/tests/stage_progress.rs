use std::time::{Duration, Instant};

use super::*;

/// A job ten minutes in, settling its words: every step before done in 30 s, the language model
/// 40 s into its third batch of eight, the rest to run.
fn settling(now: Instant) -> JobProgress {
    let mut progress = JobProgress::new(now - Duration::from_secs(600));
    for row in &mut progress.steps {
        row.state = match row.step {
            StepName::Adjudicate => StepState::Running {
                started: now - Duration::from_secs(40),
                done: 3,
                total: 8,
                message: None,
            },
            step if step < StepName::Adjudicate => StepState::Done { wall_s: 30.0 },
            _ => StepState::Pending,
        };
    }
    progress
}

fn states(rows: &[StageRow]) -> Vec<StageState> {
    rows.iter().map(|row| row.state).collect()
}

#[test]
fn a_running_job_has_eight_stages_whose_steps_are_every_step_in_order() {
    let rows = running(&JobProgress::new(Instant::now()), Instant::now());
    assert_eq!(rows.len(), 8);
    assert_eq!(
        rows.iter().map(|row| row.stage.title).collect::<Vec<_>>(),
        [
            "Read the video",
            "Separate the voices",
            "Hear the speech",
            "Settle the words",
            "Time the words",
            "Lay out the subtitles",
            "Translate on-screen text",
            "Write the subtitles",
        ]
    );
    let steps: Vec<StepName> = rows
        .iter()
        .flat_map(|row| row.steps.iter().map(|line| line.step))
        .collect();
    let expected = [
        StepName::ProbeDecode,
        StepName::ShotScan,
        StepName::Separation,
        StepName::Vad,
        StepName::AsrParakeet,
        StepName::AsrWhisper,
        StepName::DiffSheet,
        StepName::SoundEvents,
        StepName::Adjudicate,
        StepName::RedecodeParakeet,
        StepName::RedecodeWhisper,
        StepName::Readjudicate,
        StepName::SoundCues,
        StepName::Alignment,
        StepName::Review,
        StepName::Cues,
        StepName::TextDetect,
        StepName::TextRead,
        StepName::TextTrack,
        StepName::TextTranslate,
        StepName::TextReview,
        StepName::TextTypeset,
        StepName::Qc,
        StepName::Output,
    ];
    assert_eq!(steps, expected);
    assert_eq!(StepName::ALL, expected);
    assert!(
        rows.iter().all(|row| row.state == StageState::Pending),
        "a job just started has every stage to run"
    );
}

#[test]
fn stages_before_the_running_one_are_done_in_the_sum_of_their_steps() {
    let now = Instant::now();
    let rows = running(&settling(now), now);
    assert_eq!(
        states(&rows),
        [
            StageState::Done {
                seconds: Some(60.0)
            },
            StageState::Done {
                seconds: Some(60.0)
            },
            StageState::Done {
                seconds: Some(120.0)
            },
            StageState::Running {
                share: 0.375 / 5.0,
                seconds: 40.0
            },
            StageState::Pending,
            StageState::Pending,
            StageState::Pending,
            StageState::Pending,
        ]
    );
    assert_eq!(rows[3].stage.title, "Settle the words");
    assert_eq!(
        rows[3].steps[0].state,
        StageState::Running {
            share: 0.375,
            seconds: 40.0
        }
    );
    assert_eq!(rows[3].steps[1].state, StageState::Pending);
}

#[test]
fn steps_this_run_does_not_do_are_kept_and_never_to_run() {
    let now = Instant::now();
    let mut progress = JobProgress::new(now);
    for row in &mut progress.steps {
        match row.step {
            StepName::ProbeDecode => row.state = StepState::Skipped,
            // Valid from an earlier run: this run does not do it.
            StepName::ShotScan | StepName::Separation => row.stale = false,
            StepName::Vad => row.state = StepState::Done { wall_s: 2.0 },
            _ => {}
        }
    }
    let rows = running(&progress, now);
    assert_eq!(rows[0].state, StageState::Kept);
    assert_eq!(rows[0].steps[1].state, StageState::Kept);
    assert_eq!(rows[1].state, StageState::Done { seconds: Some(2.0) });
    assert_eq!(rows[2].state, StageState::Pending);
}

#[test]
fn a_stage_between_two_of_its_steps_is_still_running() {
    let now = Instant::now();
    let mut progress = JobProgress::new(now);
    for row in &mut progress.steps {
        if row.step <= StepName::AsrParakeet {
            row.state = StepState::Done { wall_s: 10.0 };
        }
    }
    let rows = running(&progress, now);
    assert_eq!(
        rows[2].state,
        StageState::Running {
            share: 0.25,
            seconds: 10.0
        }
    );
}

#[test]
fn a_failed_step_fails_its_stage() {
    let now = Instant::now();
    let mut progress = settling(now);
    if let Some(row) = progress.row_mut(StepName::Adjudicate) {
        row.state = StepState::Failed("no answer".into());
    }
    let rows = running(&progress, now);
    assert_eq!(rows[3].state, StageState::Failed);
    assert_eq!(rows[3].steps[0].state, StageState::Failed);
}

#[test]
fn a_failed_job_lists_exactly_the_steps_it_kept() {
    let failure = Failure::new(
        Some(StepName::AsrWhisper),
        "out of memory".into(),
        vec![
            (StepName::ProbeDecode, FinishedStep::StillValid),
            (StepName::Separation, FinishedStep::Done(Some(190.0))),
            (StepName::Vad, FinishedStep::Done(Some(1.0))),
            (StepName::AsrParakeet, FinishedStep::Done(None)),
        ],
    );
    let rows = failed(&failure);
    assert_eq!(
        states(&rows),
        [
            StageState::Kept,
            StageState::Done {
                seconds: Some(191.0)
            },
            StageState::Failed,
            StageState::Pending,
            StageState::Pending,
            StageState::Pending,
            StageState::Pending,
            StageState::Pending,
        ],
        "the shot scan the cues never joined does not hold Read the video open"
    );
    let lines: Vec<StageState> = rows
        .iter()
        .flat_map(|row| row.steps.iter().map(|line| line.state))
        .collect();
    let kept = lines
        .iter()
        .filter(|state| matches!(state, StageState::Kept | StageState::Done { .. }))
        .count();
    assert_eq!(kept, failure.kept_steps, "the list and the count agree");
    assert_eq!(lines[1], StageState::Pending, "the shot scan is to run");
    assert_eq!(
        &lines[4..6],
        [StageState::Done { seconds: None }, StageState::Failed]
    );
    let before_start = failed(&Failure::new(None, "locked".into(), Vec::new()));
    assert!(
        before_start
            .iter()
            .all(|row| row.state == StageState::Pending)
    );
}

#[test]
fn the_shot_scan_runs_in_the_background_and_never_holds_its_stage_open() {
    let now = Instant::now();
    let mut progress = settling(now);
    if let Some(row) = progress.row_mut(StepName::ShotScan) {
        row.state = StepState::Running {
            started: now - Duration::from_secs(500),
            done: 0,
            total: 0,
            message: None,
        };
    }
    let rows = running(&progress, now);
    assert_eq!(
        rows[0].state,
        StageState::Done {
            seconds: Some(30.0)
        },
        "Read the video is done once the video's details are read"
    );
    assert_eq!(rows[0].steps[1].state, StageState::Background);
    let mut starting = JobProgress::new(now);
    if let Some(row) = starting.row_mut(StepName::ProbeDecode) {
        row.state = StepState::Done { wall_s: 4.0 };
    }
    let rows = running(&starting, now);
    assert_eq!(
        rows[0].state,
        StageState::Done { seconds: Some(4.0) },
        "nor does a shot scan still to start"
    );
}

#[test]
fn steps_are_numbered_from_one_of_twenty_four() {
    assert_eq!(StepName::ALL.len(), 24);
    assert_eq!(step_number(StepName::ProbeDecode), 1);
    assert_eq!(step_number(StepName::Adjudicate), 9);
    assert_eq!(step_number(StepName::TextDetect), 17);
    assert_eq!(step_number(StepName::TextTypeset), 22);
    assert_eq!(step_number(StepName::Qc), 23);
    assert_eq!(step_number(StepName::Output), 24);
}

#[test]
fn the_visual_stage_reports_six_steps_and_their_measured_progress() {
    let now = Instant::now();
    let mut progress = JobProgress::new(now);
    for (step, wall_s) in [
        (StepName::TextDetect, 20.0),
        (StepName::TextRead, 30.0),
        (StepName::TextTrack, 40.0),
    ] {
        progress.row_mut(step).expect("visual step").state = StepState::Done { wall_s };
    }
    progress
        .row_mut(StepName::TextTranslate)
        .expect("translation step")
        .state = StepState::Running {
        started: now - Duration::from_secs(10),
        done: 2,
        total: 4,
        message: None,
    };
    let rows = running(&progress, now);
    assert_eq!(rows[6].stage.title, "Translate on-screen text");
    assert_eq!(
        rows[6]
            .steps
            .iter()
            .map(|line| line.step)
            .collect::<Vec<_>>(),
        [
            StepName::TextDetect,
            StepName::TextRead,
            StepName::TextTrack,
            StepName::TextTranslate,
            StepName::TextReview,
            StepName::TextTypeset,
        ]
    );
    assert_eq!(
        rows[6].state,
        StageState::Running {
            share: 3.5 / 6.0,
            seconds: 100.0,
        }
    );
    assert_eq!(
        rows[6].steps[3].state,
        StageState::Running {
            share: 0.5,
            seconds: 10.0,
        }
    );
    assert_eq!(rows[6].steps[4].state, StageState::Pending);
    assert_eq!(rows[6].steps[5].state, StageState::Pending);
    assert_eq!(rows[7].state, StageState::Pending);

    for (step, wall_s) in [
        (StepName::TextTranslate, 20.0),
        (StepName::TextReview, 5.0),
        (StepName::TextTypeset, 5.0),
    ] {
        progress.row_mut(step).expect("visual step").state = StepState::Done { wall_s };
    }
    assert_eq!(
        running(&progress, now)[6].state,
        StageState::Done {
            seconds: Some(120.0)
        }
    );
}

#[test]
fn disabled_visual_steps_are_kept_without_measured_time() {
    let now = Instant::now();
    let mut progress = JobProgress::new(now);
    for step in [
        StepName::TextDetect,
        StepName::TextRead,
        StepName::TextTrack,
        StepName::TextTranslate,
        StepName::TextReview,
        StepName::TextTypeset,
    ] {
        progress.row_mut(step).expect("visual step").stale = false;
    }
    let rows = running(&progress, now);
    assert_eq!(rows[6].state, StageState::Kept);
    assert_eq!(rows[6].steps.len(), 6);
    assert!(
        rows[6]
            .steps
            .iter()
            .all(|line| line.state == StageState::Kept)
    );
    assert_eq!(rows[7].state, StageState::Pending);
}
