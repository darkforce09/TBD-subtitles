use super::*;
use crate::job_queue::models::progress::JobProgress;
use crate::job_queue::models::queue::Failure;

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

fn running() -> JobState {
    JobState::Running(Box::new(JobProgress::new(std::time::Instant::now())))
}

fn failed() -> JobState {
    JobState::Failed(Failure {
        step: None,
        message: "boom".into(),
        kept_steps: 0,
        finished: Vec::new(),
    })
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
    q.items[0].state = JobState::Cancelled { kept_steps: 0 };
    assert_eq!(add_videos(&mut q, [PathBuf::from("a.mp4")]), 1);
    assert_eq!(q.items.len(), 2);
}

#[test]
fn a_running_job_cannot_be_removed() {
    let mut q = queue(&["a.mp4", "b.mp4"]);
    q.items[0].state = running();
    assert_eq!(remove(&mut q, 0), None);
    assert_eq!(remove(&mut q, 7), None);
    assert!(remove(&mut q, 1).is_some());
    assert_eq!(order(&q), ["a.mp4"]);
}

#[test]
fn a_removed_row_goes_back_where_it_was_with_its_correction_runs() {
    let mut q = queue(&["a", "b", "c"]);
    q.items[1].state = JobState::FinishedBefore;
    let review = queue_review(&mut q, PathBuf::from("b"));
    q.selected = Some(1);
    let removed = remove(&mut q, 1).expect("removed");
    assert_eq!(order(&q), ["a", "c"], "the row and its correction run");
    assert_eq!(q.selected, Some(2), "the row before it is selected");
    assert_eq!(removed.job().map(|item| item.id), Some(1));
    assert_eq!(removed.items.len(), 2);
    assert_eq!(restore(&mut q, removed), Ok(1));
    assert_eq!(order(&q), ["a", "b", "c", "b"]);
    assert_eq!(q.items[3].id, review);
    assert_eq!(q.selected, Some(1), "the restored row is selected");
}

#[test]
fn removing_the_last_row_selects_the_one_before() {
    let mut q = queue(&["a", "b"]);
    q.selected = Some(1);
    assert!(remove(&mut q, 1).is_some());
    assert_eq!(q.selected, Some(0));
    assert!(remove(&mut q, 0).is_some());
    assert_eq!(q.selected, None);
}

#[test]
fn a_row_whose_correction_run_runs_stays() {
    let mut q = queue(&["a"]);
    q.items[0].state = JobState::FinishedBefore;
    let review = queue_review(&mut q, PathBuf::from("a"));
    if let Some(item) = q.get_mut(review) {
        item.state = running();
    }
    assert_eq!(remove(&mut q, 0), None);
    assert_eq!(q.items.len(), 2);
}

#[test]
fn waiting_jobs_move_among_themselves() {
    let mut q = queue(&["a", "b", "c", "d"]);
    q.items[1].state = JobState::Cancelled { kept_steps: 0 };
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
fn a_dragged_job_lands_before_its_target_or_last() {
    let mut q = queue(&["a", "b", "c", "d"]);
    q.items[0].state = running();
    assert!(move_before(&mut q, 3, Some(1)));
    assert_eq!(order(&q), ["a", "d", "b", "c"]);
    assert!(move_before(&mut q, 3, None));
    assert_eq!(
        order(&q),
        ["a", "b", "c", "d"],
        "after the last waiting job"
    );
    assert!(!move_before(&mut q, 2, Some(0)), "not before a running job");
    assert!(
        !move_before(&mut q, 0, Some(1)),
        "a running job does not move"
    );
    assert!(!move_before(&mut q, 2, Some(2)));
    assert_eq!(order(&q), ["a", "b", "c", "d"]);
}

#[test]
fn trying_again_puts_an_ended_job_first_in_line() {
    let mut q = queue(&["a", "b", "c"]);
    q.items[2].state = failed();
    q.items[2].keep_settings = true;
    assert_eq!(try_again(&mut q, 2, Some(StepName::Adjudicate)), Ok(()));
    assert_eq!(order(&q), ["c", "a", "b"]);
    assert!(q.items[0].state.is_waiting());
    assert!(q.items[0].keep_settings, "it keeps its own settings");
    assert_eq!(q.items[0].rerun, [StepName::Adjudicate]);
    assert_eq!(
        try_again(&mut q, 2, None),
        Err(Refusal::NotEnded),
        "already waiting"
    );
    assert_eq!(next_waiting(&q, JobKind::Full), Some(2));
}

#[test]
fn running_again_takes_the_settings_saved_now() {
    let mut q = queue(&["a", "b"]);
    q.items[0].state = JobState::FinishedBefore;
    q.items[0].keep_settings = true;
    q.items[1].state = running();
    assert_eq!(run_again(&mut q, 0), Ok(()));
    assert!(q.items[0].state.is_waiting());
    assert!(!q.items[0].keep_settings);
    assert_eq!(
        run_again(&mut q, 1),
        Err(Refusal::NotEnded),
        "a running job"
    );
}

#[test]
fn the_queue_control_counts_only_full_runs() {
    let mut q = Queue::default();
    assert_eq!(
        queue_control(&q, false),
        QueueControl::Start {
            reason: Some("Add videos to start")
        }
    );
    add_videos(&mut q, [PathBuf::from("[Muhn Pace] Dressrosa 16.mp4")]);
    q.items[0].state = JobState::FinishedBefore;
    queue_review(&mut q, PathBuf::from("[Muhn Pace] Dressrosa 16.mp4"));
    assert_eq!(
        queue_control(&q, false),
        QueueControl::Start {
            reason: Some("Nothing is waiting")
        },
        "a waiting correction run does not enable Start"
    );
    add_videos(&mut q, [PathBuf::from("b.mp4")]);
    assert_eq!(
        queue_control(&q, false),
        QueueControl::Start { reason: None }
    );
    assert_eq!(
        queue_control(&q, true),
        QueueControl::Start {
            reason: Some("Download the models first")
        }
    );
    q.running = true;
    assert_eq!(queue_control(&q, false), QueueControl::PauseAfter);
    q.running = false;
    q.pausing = true;
    q.items[0].state = running();
    assert_eq!(
        queue_control(&q, false),
        QueueControl::Resume {
            reason: "Pauses after Dressrosa 16".into()
        }
    );
}

#[test]
fn pause_and_resume_stay_reachable_while_a_model_is_missing() {
    let mut q = queue(&["a", "b"]);
    q.items[0].state = running();
    q.running = true;
    assert_eq!(queue_control(&q, true), QueueControl::PauseAfter);
    q.running = false;
    q.pausing = true;
    assert_eq!(
        queue_control(&q, true),
        QueueControl::Resume {
            reason: "Pauses after a".into()
        }
    );
    q.pausing = false;
    assert_eq!(
        queue_control(&q, true),
        QueueControl::Start {
            reason: Some("Download the models first")
        }
    );
}

#[test]
fn a_video_is_not_put_back_while_another_run_of_it_waits() {
    let mut q = queue(&["a", "b"]);
    q.items[0].state = failed();
    add_videos(&mut q, [PathBuf::from("a")]);
    assert_eq!(try_again(&mut q, 0, None), Err(Refusal::AlreadyQueued));
    assert_eq!(run_again(&mut q, 0), Err(Refusal::AlreadyQueued));
    assert_eq!(q.items[0].state, failed(), "it stays as it was");
    let removed = remove(&mut q, 1).expect("removed");
    add_videos(&mut q, [PathBuf::from("b")]);
    assert_eq!(restore(&mut q, removed), Err(Refusal::AlreadyQueued));
    assert_eq!(order(&q), ["a", "a", "b"]);
}

#[test]
fn ended_jobs_stand_newest_first() {
    let mut q = queue(&["a", "b", "c", "d"]);
    q.items[1].state = JobState::Cancelled { kept_steps: 1 };
    newest_ended_first(&mut q, 1);
    assert_eq!(order(&q), ["a", "b", "c", "d"], "the only ended job stays");
    q.items[3].state = JobState::FinishedBefore;
    newest_ended_first(&mut q, 3);
    assert_eq!(order(&q), ["a", "d", "b", "c"]);
    q.items[0].state = failed();
    newest_ended_first(&mut q, 0);
    assert_eq!(order(&q), ["a", "d", "b", "c"], "already first");
    newest_ended_first(&mut q, 2);
    assert_eq!(
        order(&q),
        ["a", "d", "b", "c"],
        "a waiting job does not move"
    );
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
    assert_eq!(subtitle_file(&dir.join("c.mp4")), Some(dir.join("c.srt")));
    let mut q = Queue::default();
    add_videos(&mut q, [dir.join("b.mp4")]);
    assert_eq!(add_videos(&mut q, [dir.clone()]), 1);
    assert_eq!(q.items[1].video, dir.join("a.MKV"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn review_runs_wait_in_their_own_lane_and_are_queued_once() {
    let mut q = queue(&["a"]);
    let first = queue_review(&mut q, PathBuf::from("b"));
    assert_eq!(queue_review(&mut q, PathBuf::from("b")), first);
    assert_eq!(
        q.get(first).map(|i| i.corrections),
        Some(2),
        "one run carries both"
    );
    assert_eq!(q.items[0].corrections, 0, "a full run carries none");
    assert_eq!(next_waiting(&q, JobKind::Full), Some(0));
    assert_eq!(next_waiting(&q, JobKind::Review), Some(first));
}
