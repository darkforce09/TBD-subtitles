use super::*;
use crate::job_queue::models::progress::JobProgress;

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
    let failure = Failure {
        step: Some(StepName::AsrWhisper),
        message: "step asr_whisper: boom".into(),
        kept_steps: 4,
    };
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
