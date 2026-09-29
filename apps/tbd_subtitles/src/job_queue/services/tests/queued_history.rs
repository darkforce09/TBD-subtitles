use super::*;
use crate::job_queue::models::queue::{JobKind, JobState};
use crate::job_queue::services::queue_editing;

fn folder(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("tbd-queued-history-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn recording_says_whether_anything_was_added() {
    let mut history = QueuedHistory::default();
    assert!(!history.contains(Path::new("/v/a.mkv")));
    assert!(history.record([PathBuf::from("/v/a.mkv"), PathBuf::from("/v/b.mkv")]));
    assert!(history.contains(Path::new("/v/a.mkv")));
    assert!(
        !history.record([PathBuf::from("/v/a.mkv")]),
        "already there"
    );
    assert!(!history.record(Vec::new()));
}

#[test]
fn every_job_of_the_queue_is_recorded_whatever_its_state() {
    let mut queue = Queue::default();
    queue_editing::push(&mut queue, PathBuf::from("/v/a.mkv"), JobKind::Full);
    let cancelled = queue_editing::push(&mut queue, PathBuf::from("/v/b.mkv"), JobKind::Full);
    queue_editing::push(&mut queue, PathBuf::from("/v/a.mkv"), JobKind::Review);
    if let Some(item) = queue.get_mut(cancelled) {
        item.state = JobState::Cancelled { kept_steps: 0 };
    }
    let mut history = QueuedHistory::default();
    assert!(history.record_queue(&queue));
    assert!(history.contains(Path::new("/v/a.mkv")));
    assert!(history.contains(Path::new("/v/b.mkv")));
    assert!(!history.record_queue(&queue), "nothing new the second time");
}

#[test]
fn a_saved_history_loads_back() {
    let dir = folder("roundtrip");
    let path = dir.join("data").join("queued_videos.json");
    let mut history = QueuedHistory::default();
    history.record([PathBuf::from("/v/b.mkv"), PathBuf::from("/v/a.mkv")]);
    save(&path, &history).expect("save");
    assert_eq!(load(&path), history);
    assert!(!dir.join("data").join("queued_videos.json.part").exists());
    history.record([PathBuf::from("/v/c.mkv")]);
    save(&path, &history).expect("save again");
    assert_eq!(load(&path), history);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_file_is_an_empty_history() {
    let dir = folder("missing");
    assert_eq!(
        load(&dir.join("queued_videos.json")),
        QueuedHistory::default()
    );
}

#[test]
fn a_broken_file_is_an_empty_history() {
    let dir = folder("broken");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("queued_videos.json");
    std::fs::write(&path, "{ not json").expect("file");
    assert_eq!(load(&path), QueuedHistory::default());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_default_file_is_in_the_app_data_folder() {
    if let Some(path) = default_path() {
        assert_eq!(
            path.file_name(),
            Some(std::ffi::OsStr::new("queued_videos.json"))
        );
        assert!(
            path.parent()
                .is_some_and(|dir| dir.ends_with("tbd-subtitles"))
        );
    }
}
