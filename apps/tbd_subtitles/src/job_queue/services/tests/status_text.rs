use std::path::PathBuf;
use std::time::Duration;

use job_model::StepName;

use super::*;
use crate::job_queue::models::progress::StepState;
use crate::job_queue::models::queue::{Failure, JobResult};
use crate::job_queue::services::queue_editing::{add_videos, queue_review};
use crate::job_queue::services::sidebar_rows;

/// Each row's status line, in sidebar order.
fn lines(queue: &Queue) -> Vec<String> {
    let now = Instant::now();
    sidebar_rows::rows(queue)
        .iter()
        .map(|row| {
            let item = queue.get(row.id).expect("the row's job");
            status(row, item, queue, &time_left::pilot_rates(), now)
        })
        .collect()
}

fn queue(videos: &[&str]) -> Queue {
    let mut q = Queue::default();
    add_videos(&mut q, videos.iter().map(PathBuf::from));
    q
}

#[test]
fn waiting_rows_show_their_place_only_while_the_queue_runs() {
    let mut q = queue(&["a", "b", "c"]);
    assert_eq!(lines(&q), ["Waiting · next in line", "Waiting", "Waiting"]);
    q.running = true;
    assert_eq!(
        lines(&q),
        [
            "Waiting · next in line",
            "Waiting · 2nd in line",
            "Waiting · 3rd in line"
        ]
    );
}

#[test]
fn ended_rows_say_how_they_ended() {
    let mut q = queue(&["a", "b", "c", "d", "e"]);
    q.items[0].state = JobState::Failed(Failure {
        step: Some(StepName::AsrWhisper),
        message: "boom".into(),
        kept_steps: 4,
    });
    q.items[1].state = JobState::Cancelled { kept_steps: 9 };
    q.items[2].state = JobState::FinishedBefore;
    q.items[3].state = JobState::Finished(JobResult {
        subtitles: PathBuf::from("d.srt"),
        work_dir: PathBuf::from("work"),
        failures: vec!["reading speed".into()],
        findings: 3,
        wall_s: 60.0,
    });
    q.items[4].state = JobState::Failed(Failure {
        step: None,
        message: "boom".into(),
        kept_steps: 0,
    });
    assert_eq!(
        lines(&q),
        [
            "Failed at Hear the speech",
            "Cancelled · 9 finished steps kept",
            "Subtitles ready",
            "Needs attention · 1 problem",
            "Failed",
        ]
    );
}

#[test]
fn a_row_with_waiting_corrections_is_updating() {
    let mut q = queue(&["a"]);
    q.items[0].state = JobState::FinishedBefore;
    queue_review(&mut q, PathBuf::from("a"));
    queue_review(&mut q, PathBuf::from("a"));
    assert_eq!(lines(&q), ["Updating subtitles · 2 corrections"]);
}

#[test]
fn a_running_row_says_what_its_stage_does_and_the_time_left() {
    let mut q = queue(&["a"]);
    let now = Instant::now();
    let mut progress = JobProgress::new(now - Duration::from_secs(120));
    progress.duration_s = Some(1500.0);
    q.items[0].state = JobState::Running(Box::new(progress.clone()));
    assert_eq!(lines(&q), ["Starting…"]);
    if let Some(row) = progress.row_mut(StepName::Adjudicate) {
        row.state = StepState::Running {
            started: now,
            done: 0,
            total: 0,
            message: None,
        };
    }
    q.items[0].state = JobState::Running(Box::new(progress.clone()));
    let line = &lines(&q)[0];
    assert!(line.starts_with("Settling the words · "), "{line}");
    assert!(line.ends_with(" left"), "{line}");
    progress.cancelling = true;
    q.items[0].state = JobState::Running(Box::new(progress));
    assert_eq!(lines(&q), ["Stopping…"]);
}
