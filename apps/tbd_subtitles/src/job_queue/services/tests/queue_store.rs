use super::*;
use crate::job_queue::models::progress::JobProgress;

#[test]
fn a_saved_queue_loads_back_with_the_running_job_waiting() {
    let dir = std::env::temp_dir().join(format!("tbd-queue-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("queue.json");
    let mut queue = Queue::default();
    for video in ["a.mp4", "b.mp4", "c.mp4", "d.mp4"] {
        queue_editing::push(&mut queue, PathBuf::from(video), JobKind::Full);
    }
    queue.items[0].state = JobState::Running(Box::new(JobProgress::new(std::time::Instant::now())));
    queue.items[1].state = JobState::Cancelled;
    queue.items[2].state = JobState::Failed("boom".into());
    queue.items[3].kind = JobKind::Review;
    save(&path, &queue).expect("save");
    let back = load(&path).expect("load");
    assert_eq!(back.items.len(), 4);
    assert!(back.items[0].state.is_waiting());
    assert_eq!(back.items[1].state, JobState::Cancelled);
    assert!(matches!(back.items[2].state, JobState::Failed(_)));
    assert_eq!(back.items[3].kind, JobKind::Review);
    assert!(!back.running, "a loaded queue waits for Start");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_file_is_an_empty_queue_and_a_broken_one_an_error() {
    assert_eq!(
        load(Path::new("/no/such/queue.json")).expect("empty"),
        Queue::default()
    );
    let dir = std::env::temp_dir().join(format!("tbd-queue-broken-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(dir.join("queue.json"), "{").expect("write");
    assert!(load(&dir.join("queue.json")).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
