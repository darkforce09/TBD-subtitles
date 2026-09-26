use std::path::PathBuf;
use std::time::{Duration, Instant};

use job_model::StepName;
use job_model::job::StepMeasure;

use super::*;

#[test]
fn events_move_each_step_from_pending_to_done() {
    let start = Instant::now();
    let mut p = JobProgress::new(start);
    apply(
        &mut p,
        Progress::JobStarted {
            video: PathBuf::from("a.mp4"),
            work_dir: PathBuf::from("/work/a"),
            stale: vec![StepName::Cues, StepName::Qc, StepName::Output],
        },
        start,
    );
    assert_eq!(p.work_dir, Some(PathBuf::from("/work/a")));
    assert_eq!(p.steps.iter().filter(|r| r.stale).count(), 3);
    apply(&mut p, Progress::StepSkipped(StepName::Alignment), start);
    let later = start + Duration::from_secs(3);
    apply(&mut p, Progress::StepStarted(StepName::Cues), later);
    apply(
        &mut p,
        Progress::StepAdvanced {
            step: StepName::Cues,
            done: 2,
            total: 4,
        },
        later,
    );
    let row = |p: &JobProgress, s| p.steps.iter().find(|r| r.step == s).expect("row").clone();
    assert!(matches!(
        row(&p, StepName::Cues).state,
        StepState::Running {
            done: 2,
            total: 4,
            ..
        }
    ));
    apply(
        &mut p,
        Progress::StepFinished {
            step: StepName::Cues,
            measure: StepMeasure {
                wall_s: 1.5,
                ..StepMeasure::default()
            },
        },
        later,
    );
    assert_eq!(
        row(&p, StepName::Cues).state,
        StepState::Done { wall_s: 1.5 }
    );
    assert_eq!(row(&p, StepName::Alignment).state, StepState::Skipped);
    apply(
        &mut p,
        Progress::StepFailed {
            step: StepName::Qc,
            message: "boom".into(),
        },
        later,
    );
    assert_eq!(
        row(&p, StepName::Qc).state,
        StepState::Failed("boom".into())
    );
    apply(&mut p, Progress::JobDuration(1853.7), later);
    assert_eq!(p.duration_s, Some(1853.7));
}
