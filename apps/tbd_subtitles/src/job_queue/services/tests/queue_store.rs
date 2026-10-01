use super::*;
use crate::job_queue::models::progress::{FinishedStep, JobProgress};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tbd-queue-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

#[test]
fn a_saved_queue_loads_back_with_the_running_job_waiting() {
    let dir = scratch("store");
    let path = dir.join("queue.json");
    let mut queue = Queue::default();
    for video in ["a.mp4", "b.mp4", "c.mp4", "d.mp4"] {
        queue_editing::push(&mut queue, PathBuf::from(video), JobKind::Full);
    }
    queue.items[0].state = JobState::Running(Box::new(JobProgress::new(std::time::Instant::now())));
    queue.items[0].keep_settings = true;
    queue.items[0].rerun = vec![StepName::Adjudicate];
    queue.items[1].state = JobState::Cancelled { kept_steps: 9 };
    let failure = Failure::new(
        Some(StepName::AsrWhisper),
        "step asr_whisper: boom".into(),
        vec![
            (StepName::ProbeDecode, FinishedStep::StillValid),
            (StepName::ShotScan, FinishedStep::Done(Some(12.5))),
            (StepName::Separation, FinishedStep::Done(Some(190.0))),
            (StepName::Vad, FinishedStep::Done(Some(1.0))),
        ],
    );
    queue.items[2].state = JobState::Failed(failure.clone());
    queue.items[3].kind = JobKind::Review;
    queue.items[3].corrections = 2;
    save(&path, &queue).expect("save");
    let back = load(&path).expect("load");
    assert_eq!(back.items.len(), 4);
    assert!(back.items[0].state.is_waiting());
    assert!(
        back.items[0].keep_settings,
        "a started job keeps its settings"
    );
    assert_eq!(back.items[0].rerun, [StepName::Adjudicate]);
    assert_eq!(back.items[1].state, JobState::Cancelled { kept_steps: 9 });
    assert_eq!(back.items[2].state, JobState::Failed(failure));
    assert_eq!(back.items[3].kind, JobKind::Review);
    assert_eq!(back.items[3].corrections, 2);
    assert!(!back.items[3].keep_settings);
    assert!(!back.running, "a loaded queue waits for Start");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_written_before_the_new_fields_still_loads() {
    let dir = scratch("old");
    let path = dir.join("queue.json");
    std::fs::write(
        &path,
        r#"[{"video":"a.mp4","state":"failed"},
            {"video":"b.mp4","state":"cancelled","review":true},
            {"video":"c.mp4","state":"waiting"},
            {"video":"d.mp4","state":"finished"}]"#,
    )
    .expect("write");
    let back = load(&path).expect("load");
    assert_eq!(
        back.items[0].state,
        JobState::Failed(Failure {
            step: None,
            message: "failed in an earlier window".into(),
            kept_steps: 0,
            finished: Vec::new(),
        })
    );
    assert_eq!(back.items[1].state, JobState::Cancelled { kept_steps: 0 });
    assert_eq!(back.items[1].kind, JobKind::Review);
    let keeps: Vec<bool> = back.items.iter().map(|i| i.keep_settings).collect();
    assert_eq!(
        keeps,
        [true, true, false, true],
        "a job that no longer waits has started"
    );
    for item in &back.items {
        assert!(item.rerun.is_empty() && item.corrections == 0);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_file_is_an_empty_queue_and_a_broken_one_an_error() {
    assert_eq!(
        load(Path::new("/no/such/queue.json")).expect("empty"),
        Queue::default()
    );
    let dir = scratch("broken");
    std::fs::write(dir.join("queue.json"), "{").expect("write");
    assert!(load(&dir.join("queue.json")).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failure_an_older_window_kept_reads_its_finished_steps_from_its_job_database() {
    let dir = scratch("finished");
    let video = dir.join("Dressrosa 19.mp4");
    std::fs::write(&video, b"video").expect("video");
    let path = dir.join("queue.json");
    std::fs::write(
        &path,
        format!(
            r#"[{{"video":{a:?},"state":"failed","failed_step":"asr_whisper","kept_steps":5}},
                {{"video":{b:?},"state":"failed","failed_step":"asr_whisper","kept_steps":3}}]"#,
            a = video.display().to_string(),
            b = dir.join("gone.mp4").display().to_string(),
        ),
    )
    .expect("write");
    let work = dir.join("work");
    let job = work.join(pipeline::work_dir::job_id(
        &std::fs::canonicalize(&video).expect("c"),
    ));
    std::fs::create_dir_all(&job).expect("job");
    let step = |wall_s: f64| job_model::job::StepRecord {
        fingerprint: String::new(),
        finished_ns: 0,
        measure: job_model::job::StepMeasure {
            wall_s,
            ..Default::default()
        },
    };
    let record = job_model::job::JobRecord {
        video: video.display().to_string(),
        video_size: 5,
        video_modified_s: 0,
        settings: job_model::job::JobSettings::with_glossary(vec![]),
        models_dir: None,
        corrections: None,
    };
    let store = pipeline::work_dir::JobStore::open(&pipeline::work_dir::WorkDir::new(&job))
        .expect("the store");
    store.put_job_record(&record).expect("the record");
    for (name, done) in [
        (StepName::ProbeDecode, step(4.0)),
        (StepName::Separation, step(190.0)),
        (StepName::Vad, step(1.0)),
        (StepName::AsrParakeet, step(16.0)),
        (StepName::AsrWhisper, step(3.0)),
    ] {
        store.put_step_record(name, &done).expect("the step");
    }
    drop(store);
    let mut queue = load(&path).expect("load");
    read_finished_steps(&mut queue, &work);
    let JobState::Failed(read) = &queue.items[0].state else {
        panic!("failed: {:?}", queue.items[0].state);
    };
    assert_eq!(
        read.finished,
        [
            (StepName::ProbeDecode, FinishedStep::Done(Some(4.0))),
            (StepName::Separation, FinishedStep::Done(Some(190.0))),
            (StepName::Vad, FinishedStep::Done(Some(1.0))),
            (StepName::AsrParakeet, FinishedStep::Done(Some(16.0))),
        ],
        "the steps before the failed one, the shot scan never joined"
    );
    assert_eq!(read.kept_steps, 4, "it keeps as many as it lists");
    let JobState::Failed(guessed) = &queue.items[1].state else {
        panic!("failed: {:?}", queue.items[1].state);
    };
    assert_eq!(
        guessed.finished,
        [
            (StepName::ProbeDecode, FinishedStep::Done(None)),
            (StepName::ShotScan, FinishedStep::Done(None)),
            (StepName::Separation, FinishedStep::Done(None)),
        ],
        "with no job record, the steps it kept, in a time not known"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_busy_job_is_saved_waiting_so_the_next_window_tries_it_again() {
    let dir = scratch("busy");
    let path = dir.join("queue.json");
    let mut queue = Queue::default();
    queue_editing::push(&mut queue, PathBuf::from("a.mp4"), JobKind::Full);
    queue.items[0].state = JobState::Busy {
        owner: Some(4242),
        since: std::time::Instant::now(),
    };
    queue.items[0].keep_settings = true;
    save(&path, &queue).expect("save");
    let back = load(&path).expect("load");
    assert!(back.items[0].state.is_waiting());
    assert!(back.items[0].keep_settings);
    let _ = std::fs::remove_dir_all(&dir);
}
