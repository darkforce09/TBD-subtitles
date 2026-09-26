use super::*;

fn queue(videos: &[&str]) -> Queue {
    let mut q = Queue::default();
    add_videos(&mut q, videos.iter().map(PathBuf::from));
    q
}

fn order(q: &Queue) -> Vec<String> {
    q.items
        .iter()
        .map(|i| i.video.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn adding_skips_queued_videos_and_empty_paths_and_keeps_order() {
    let mut q = queue(&["a.mp4"]);
    let added = add_videos(
        &mut q,
        ["b.mp4", "a.mp4", "", "c.mp4", "b.mp4"].map(PathBuf::from),
    );
    assert_eq!(added, 2);
    assert_eq!(order(&q), ["a.mp4", "b.mp4", "c.mp4"]);
    let ids: Vec<JobId> = q.items.iter().map(|i| i.id).collect();
    assert_eq!(ids, [0, 1, 2]);
}

#[test]
fn a_finished_video_can_be_queued_again() {
    let mut q = queue(&["a.mp4"]);
    q.items[0].state = JobState::Cancelled;
    assert_eq!(add_videos(&mut q, [PathBuf::from("a.mp4")]), 1);
    assert_eq!(q.items.len(), 2);
}

#[test]
fn a_running_job_cannot_be_removed() {
    let mut q = queue(&["a.mp4", "b.mp4"]);
    q.items[0].state = JobState::Running(Box::new(
        crate::job_queue::models::progress::JobProgress::new(std::time::Instant::now()),
    ));
    q.selected = Some(1);
    assert_eq!(remove(&mut q, 0), None);
    assert_eq!(remove(&mut q, 1), Some(PathBuf::from("b.mp4")));
    assert_eq!(q.selected, None);
    assert_eq!(remove(&mut q, 7), None);
}

#[test]
fn waiting_jobs_move_among_themselves() {
    let mut q = queue(&["a", "b", "c", "d"]);
    q.items[1].state = JobState::Cancelled;
    move_job(&mut q, 3, Move::Up);
    assert_eq!(order(&q), ["a", "b", "d", "c"]);
    move_job(&mut q, 0, Move::Down);
    assert_eq!(order(&q), ["b", "d", "a", "c"]);
    move_job(&mut q, 2, Move::Top);
    assert_eq!(order(&q), ["b", "c", "d", "a"], "first among the waiting");
    move_job(&mut q, 2, Move::Top);
    assert_eq!(order(&q), ["b", "c", "d", "a"], "the first stays");
    move_job(&mut q, 1, Move::Up);
    assert_eq!(
        order(&q),
        ["b", "c", "d", "a"],
        "a cancelled job does not move"
    );
}

#[test]
fn only_an_ended_job_is_retried_and_the_first_waiting_runs_next() {
    let mut q = queue(&["a", "b"]);
    q.items[0].state = JobState::Failed("boom".into());
    assert_eq!(next_waiting(&q), Some(1));
    assert!(retry(&mut q, 0));
    assert!(!retry(&mut q, 1), "already waiting");
    assert_eq!(next_waiting(&q), Some(0));
}

#[test]
fn a_folder_stands_for_its_videos_without_subtitles() {
    let dir = std::env::temp_dir().join(format!("tbd-queue-folder-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("done")).expect("dir");
    for name in ["b.mp4", "a.MKV", "c.mp4", "c.srt", "notes.txt"] {
        std::fs::write(dir.join(name), b"x").expect("file");
    }
    std::fs::write(dir.join("done").join("d.mp4"), b"x").expect("file");
    assert_eq!(
        videos_in_folder(&dir),
        [dir.join("a.MKV"), dir.join("b.mp4")],
        "sorted, subtitled and non-video files and subfolders left out"
    );
    let mut q = Queue::default();
    add_videos(&mut q, [dir.join("b.mp4")]);
    assert_eq!(add_videos(&mut q, [dir.clone()]), 1);
    assert_eq!(q.items[1].video, dir.join("a.MKV"));
    let _ = std::fs::remove_dir_all(&dir);
}
