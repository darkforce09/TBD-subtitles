use std::path::PathBuf;

use super::*;
use crate::job_queue::models::progress::JobProgress;
use crate::job_queue::models::queue::Failure;
use crate::job_queue::services::queue_editing::{add_videos, queue_review};

fn queue(videos: &[&str]) -> Queue {
    let mut q = Queue::default();
    add_videos(&mut q, videos.iter().map(PathBuf::from));
    q
}

fn names(rows: &[SidebarRow]) -> Vec<(Section, String)> {
    rows.iter().map(|r| (r.section, r.name.clone())).collect()
}

fn running() -> JobState {
    JobState::Running(Box::new(JobProgress::new(std::time::Instant::now())))
}

#[test]
fn rows_come_in_sections_now_up_next_done_in_queue_order() {
    let mut q = queue(&["/v/a.mp4", "/v/b.mkv", "/v/c.mp4", "/v/d.mp4"]);
    q.items[0].state = JobState::FinishedBefore;
    q.items[2].state = running();
    q.items[3].state = JobState::Cancelled { kept_steps: 3 };
    let rows = rows(&q);
    assert_eq!(
        names(&rows),
        [
            (Section::Now, "c".to_string()),
            (Section::UpNext, "b".to_string()),
            (Section::Done, "a".to_string()),
            (Section::Done, "d".to_string()),
        ]
    );
    assert_eq!(rows[1].place, Some(1));
    assert!(rows[1].draggable && rows[1].removable);
    assert!(
        !rows[0].removable && !rows[0].draggable,
        "a running row stays"
    );
    assert!(!rows[2].draggable);
}

#[test]
fn places_count_the_waiting_full_runs_in_order() {
    let mut q = queue(&["a", "b", "c"]);
    q.items[0].state = running();
    let rows = rows(&q);
    let places: Vec<Option<usize>> = rows.iter().map(|r| r.place).collect();
    assert_eq!(places, [None, Some(1), Some(2)]);
}

#[test]
fn correction_runs_fold_into_their_videos_row() {
    let mut q = queue(&["/v/a.mp4", "/v/b.mp4"]);
    q.items[0].state = JobState::FinishedBefore;
    let first = queue_review(&mut q, PathBuf::from("/v/a.mp4"));
    if let Some(item) = q.get_mut(first) {
        item.state = running();
    }
    let second = queue_review(&mut q, PathBuf::from("/v/a.mp4"));
    queue_review(&mut q, PathBuf::from("/v/a.mp4"));
    let rows = rows(&q);
    assert_eq!(rows.len(), 2, "one row per video: {rows:?}");
    let done = &rows[1];
    assert_eq!(done.name, "a");
    assert_eq!(done.section, Section::Done);
    assert_eq!(done.folded, [first, second]);
    assert_eq!(
        done.fold,
        Some(ReviewFold {
            corrections: 3,
            running: Some(first)
        })
    );
    assert!(!done.removable, "its correction run runs");
}

#[test]
fn a_finished_correction_run_folds_quietly() {
    let mut q = queue(&["a"]);
    q.items[0].state = JobState::FinishedBefore;
    let review = queue_review(&mut q, PathBuf::from("a"));
    if let Some(item) = q.get_mut(review) {
        item.state = JobState::FinishedBefore;
    }
    let rows = rows(&q);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].fold, None);
    assert_eq!(rows[0].folded, [review]);
    assert!(rows[0].removable);
}

#[test]
fn a_failed_or_lone_correction_run_keeps_its_own_row() {
    let mut q = queue(&["a"]);
    q.items[0].state = JobState::FinishedBefore;
    let failed = queue_review(&mut q, PathBuf::from("a"));
    if let Some(item) = q.get_mut(failed) {
        item.state = JobState::Failed(Failure {
            step: None,
            message: "boom".into(),
            kept_steps: 0,
            finished: Vec::new(),
        });
    }
    let lone = queue_review(&mut q, PathBuf::from("b"));
    let rows = rows(&q);
    assert_eq!(
        names(&rows),
        [
            (Section::UpNext, "b · 1 correction".to_string()),
            (Section::Done, "a".to_string()),
            (Section::Done, "a · 1 correction".to_string()),
        ]
    );
    assert_eq!(rows[0].id, lone);
    assert_eq!(rows[0].place, None, "a correction run is not in line");
    assert!(!rows[0].draggable);
}

#[test]
fn a_correction_run_folds_into_the_finished_run_of_a_video_queued_again() {
    let mut q = queue(&["a"]);
    q.items[0].state = JobState::FinishedBefore;
    add_videos(&mut q, [PathBuf::from("a")]);
    let review = queue_review(&mut q, PathBuf::from("a"));
    let rows = rows(&q);
    assert_eq!(rows.len(), 2);
    assert!(rows[0].folded.is_empty(), "the waiting run");
    assert_eq!(rows[1].folded, [review], "the finished run");
}

#[test]
fn done_rows_list_the_newest_ended_first() {
    use crate::job_queue::services::queue_editing::newest_ended_first;
    let mut q = queue(&["a", "b", "c"]);
    q.items[0].state = JobState::FinishedBefore;
    newest_ended_first(&mut q, 0);
    q.items[2].state = JobState::Cancelled { kept_steps: 2 };
    newest_ended_first(&mut q, 2);
    let done: Vec<String> = rows(&q)
        .into_iter()
        .filter(|row| row.section == Section::Done)
        .map(|row| row.name)
        .collect();
    assert_eq!(done, ["c", "a"]);
}
