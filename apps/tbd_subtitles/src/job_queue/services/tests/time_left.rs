use std::time::{Duration, Instant};

use job_model::job::{JobRecord, JobSettings, StepMeasure, StepRecord};

use super::*;

fn running(p: &mut JobProgress, step: StepName, started: Instant, done: usize, total: usize) {
    if let Some(row) = p.row_mut(step) {
        row.state = StepState::Running {
            started,
            done,
            total,
            message: None,
        };
    }
}

#[test]
fn nothing_is_estimated_before_the_video_length_is_known() {
    let p = JobProgress::new(Instant::now());
    assert_eq!(estimate(&p, &pilot_rates(), Instant::now()), None);
}

#[test]
fn a_new_job_of_the_pilot_length_has_the_pilot_time_left() {
    let now = Instant::now();
    let mut p = JobProgress::new(now);
    p.duration_s = Some(PILOT_VIDEO_S);
    let (left, share) = estimate(&p, &pilot_rates(), now).expect("estimate");
    let pilot: f64 = PILOT.iter().map(|(_, s)| s).sum();
    assert!((left - pilot).abs() < 0.01, "{left} vs {pilot}");
    assert_eq!(share, 0.0);
}

#[test]
fn done_skipped_and_the_shot_scan_add_nothing_and_a_step_keeps_its_own_pace() {
    let start = Instant::now();
    let mut p = JobProgress::new(start);
    p.duration_s = Some(PILOT_VIDEO_S);
    for row in &mut p.steps {
        row.stale = matches!(row.step, StepName::Separation | StepName::ShotScan);
    }
    let now = start + Duration::from_secs(20);
    running(&mut p, StepName::Separation, start, 1, 4);
    let (left, share) = estimate(&p, &pilot_rates(), now).expect("estimate");
    assert!((left - 60.0).abs() < 0.01, "a quarter took 20 s: {left}");
    assert!(share > 0.0 && share < 1.0);
    if let Some(row) = p.row_mut(StepName::Separation) {
        row.state = StepState::Done { wall_s: 80.0 };
    }
    assert_eq!(estimate(&p, &pilot_rates(), now), Some((0.0, 1.0)));
}

#[test]
fn history_replaces_the_pilot_rate_of_each_step_it_measured() {
    let root = std::env::temp_dir().join(format!("tbd-rates-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let job = root.join("job-a");
    std::fs::create_dir_all(&job).expect("dir");
    let mut record = JobRecord {
        video: "a.mp4".into(),
        video_size: 1,
        video_modified_s: 0,
        settings: JobSettings::with_glossary(vec![]),
        models_dir: None,
        corrections: None,
        steps: Default::default(),
    };
    record.steps.insert(
        StepName::Separation,
        StepRecord {
            fingerprint: String::new(),
            finished_ns: 0,
            measure: StepMeasure {
                wall_s: 200.0,
                ..StepMeasure::default()
            },
        },
    );
    std::fs::write(
        job.join("job.json"),
        serde_json::to_string(&record).expect("json"),
    )
    .expect("write");
    let probe = r#"{"probe":{"duration_s":1000.0,"video":null,"audio":[]},"track":{"index":1,"audio_position":0,"codec":"aac","language":null,"channels":2,"sample_rate":48000,"start_time_s":0.0},"samples":0}"#;
    std::fs::write(job.join("probe.json"), probe).expect("write");
    let rates = from_history(&root);
    assert_eq!(rates.per_step.get(&StepName::Separation), Some(&0.2));
    assert_eq!(
        rates.per_step.get(&StepName::Alignment),
        pilot_rates().per_step.get(&StepName::Alignment)
    );
    let _ = std::fs::remove_dir_all(&root);
}
