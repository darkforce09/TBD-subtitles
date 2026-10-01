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
fn a_new_job_has_the_pilot_audio_time_and_initial_visual_time_left() {
    let now = Instant::now();
    let mut p = JobProgress::new(now);
    p.duration_s = Some(PILOT_VIDEO_S);
    let (left, share) = estimate(&p, &pilot_rates(), now).expect("estimate");
    let pilot: f64 = PILOT.iter().map(|(_, s)| s).sum();
    let initial_visual = PILOT_VIDEO_S * 1.25;
    assert!(
        (left - pilot - initial_visual).abs() < 0.01,
        "{left} vs audio {pilot} and visual {initial_visual}"
    );
    assert_eq!(share, 0.0);
}

#[test]
fn every_visual_step_has_a_positive_estimate_without_measured_history() {
    let now = Instant::now();
    for (step, seconds_for_thousand_seconds) in [
        (StepName::TextDetect, 200.0),
        (StepName::TextRead, 150.0),
        (StepName::TextTrack, 250.0),
        (StepName::TextTranslate, 200.0),
        (StepName::TextReview, 10.0),
        (StepName::TextMask, 50.0),
        (StepName::TextInpaint, 100.0),
        (StepName::TextCompose, 20.0),
        (StepName::TextVerify, 10.0),
        (StepName::TextTypeset, 10.0),
        (StepName::LocalizedVideo, 250.0),
    ] {
        let mut progress = JobProgress::new(now);
        progress.duration_s = Some(1000.0);
        for row in &mut progress.steps {
            row.stale = row.step == step;
        }
        let rates = Rates::default();
        let (left, share) = estimate(&progress, &rates, now).expect("known duration");
        assert!(left.is_finite() && left > 0.0, "{step}: {left}");
        assert!((left - seconds_for_thousand_seconds).abs() < 0.01);
        assert_eq!(share, 0.0);
        progress.duration_s = Some(2000.0);
        let (twice, _) = estimate(&progress, &rates, now).expect("known duration");
        assert!((twice - left * 2.0).abs() < 0.01, "{step}");
    }
}

#[test]
fn disabled_visual_steps_add_no_time_even_with_measured_history() {
    let now = Instant::now();
    let mut progress = JobProgress::new(now);
    progress.duration_s = Some(PILOT_VIDEO_S);
    let mut rates = pilot_rates();
    for step in [
        StepName::TextDetect,
        StepName::TextRead,
        StepName::TextTrack,
        StepName::TextTranslate,
        StepName::TextReview,
        StepName::TextMask,
        StepName::TextInpaint,
        StepName::TextCompose,
        StepName::TextVerify,
        StepName::TextTypeset,
        StepName::LocalizedVideo,
    ] {
        progress.row_mut(step).expect("visual step").stale = false;
        rates.per_step.insert(step, 100.0);
    }
    let (left, share) = estimate(&progress, &rates, now).expect("known duration");
    let audio: f64 = PILOT.iter().map(|(_, seconds)| seconds).sum();
    assert!((left - audio).abs() < 0.01, "{left} vs {audio}");
    assert_eq!(share, 0.0);
    for row in &mut progress.steps {
        row.stale = false;
    }
    assert_eq!(estimate(&progress, &rates, now), Some((0.0, 0.0)));
}

/// Settings with on-screen translation `enabled` and the localized video `localized`.
fn settings(enabled: bool, localized: bool) -> JobSettings {
    let mut settings = JobSettings::with_glossary(vec![]);
    settings.onscreen_text.enabled = enabled;
    settings.onscreen_text.localized_video = localized;
    settings
}

const REPLACEMENT: [StepName; 5] = [
    StepName::TextMask,
    StepName::TextInpaint,
    StepName::TextCompose,
    StepName::TextVerify,
    StepName::LocalizedVideo,
];

#[test]
fn the_steps_a_job_leaves_idle_follow_its_settings() {
    assert!(idle_steps(&settings(true, true)).is_empty());
    assert_eq!(idle_steps(&settings(true, false)), REPLACEMENT);
    let off = idle_steps(&settings(false, true));
    let visual: Vec<StepName> = StepName::ALL
        .into_iter()
        .filter(|step| step.stage() == job_model::StageName::OnscreenText)
        .chain([StepName::LocalizedVideo])
        .collect();
    assert_eq!(off, visual, "translation off leaves every visual step idle");
    for step in [StepName::Separation, StepName::Output, StepName::Qc] {
        assert!(works_in(step, &settings(false, false)), "{step}");
    }
}

#[test]
fn idle_steps_add_no_time_left_even_while_stale() {
    let now = Instant::now();
    let rates = pilot_rates();
    let audio: f64 = PILOT.iter().map(|(_, seconds)| seconds).sum();
    let mut progress = JobProgress::new(now);
    progress.duration_s = Some(PILOT_VIDEO_S);
    progress.idle = idle_steps(&settings(false, false));
    let (left, _) = estimate(&progress, &rates, now).expect("known duration");
    assert!((left - audio).abs() < 0.01, "{left} vs {audio}");
    progress.idle = idle_steps(&settings(true, false));
    let (left, _) = estimate(&progress, &rates, now).expect("known duration");
    let reading = PILOT_VIDEO_S * (0.2 + 0.15 + 0.25 + 0.2 + 0.01 + 0.01);
    assert!(
        (left - audio - reading).abs() < 0.01,
        "the replacement steps add nothing: {left}"
    );
    progress.idle = idle_steps(&settings(true, true));
    let (left, _) = estimate(&progress, &rates, now).expect("known duration");
    assert!(
        (left - audio - PILOT_VIDEO_S * 1.25).abs() < 0.01,
        "the localized video adds its steps: {left}"
    );
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
    let record = JobRecord {
        video: "a.mp4".into(),
        video_size: 1,
        video_modified_s: 0,
        settings: JobSettings::with_glossary(vec![]),
        models_dir: None,
        corrections: None,
    };
    stored_job(&job, &record, &[(StepName::Separation, 200.0)]);
    let rates = from_history(&root);
    assert_eq!(rates.per_step.get(&StepName::Separation), Some(&0.2));
    assert_eq!(
        rates.per_step.get(&StepName::Alignment),
        pilot_rates().per_step.get(&StepName::Alignment)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn history_ignores_the_time_of_a_step_the_job_left_idle() {
    let root = std::env::temp_dir().join(format!("tbd-idle-rates-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let job = root.join("job-a");
    std::fs::create_dir_all(&job).expect("dir");
    let record = JobRecord {
        video: "a.mp4".into(),
        video_size: 1,
        video_modified_s: 0,
        settings: settings(true, false),
        models_dir: None,
        corrections: None,
    };
    stored_job(
        &job,
        &record,
        &[(StepName::TextMask, 0.01), (StepName::TextDetect, 300.0)],
    );
    let rates = from_history(&root);
    assert_eq!(rates.per_step.get(&StepName::TextDetect), Some(&0.3));
    assert_eq!(
        rates.per_step.get(&StepName::TextMask),
        None,
        "an idle run teaches no rate"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// The job in `job`, stored as a finished run leaves it: its record, the steps with their wall
/// times, and a probe of 1000 seconds.
fn stored_job(job: &std::path::Path, record: &JobRecord, steps: &[(StepName, f64)]) {
    let store = pipeline::work_dir::JobStore::open(&pipeline::work_dir::WorkDir::new(job))
        .expect("the store");
    store.put_job_record(record).expect("the record");
    for (step, wall_s) in steps {
        let done = StepRecord {
            fingerprint: String::new(),
            finished_ns: 0,
            measure: StepMeasure {
                wall_s: *wall_s,
                ..StepMeasure::default()
            },
        };
        store.put_step_record(*step, &done).expect("the step");
    }
    let probe = r#"{"probe":{"duration_s":1000.0,"video":null,"audio":[]},"track":{"index":1,"audio_position":0,"codec":"aac","language":null,"channels":2,"sample_rate":48000,"start_time_s":0.0},"samples":0}"#;
    let probe: ProbeDecoded = serde_json::from_str(probe).expect("the probe");
    store
        .put_output(StepName::ProbeDecode, None, &probe)
        .expect("the probe");
}
