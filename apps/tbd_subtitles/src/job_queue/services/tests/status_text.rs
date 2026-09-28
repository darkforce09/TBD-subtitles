use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use job_model::StepName;

use super::*;
use crate::job_queue::models::progress::StepState;
use crate::job_queue::models::queue::{Failure, JobId, JobResult};
use crate::job_queue::services::queue_editing::{add_videos, queue_review};
use crate::job_queue::services::sidebar_rows;

/// Each row's status line, in sidebar order, with no finished job's files read.
fn lines(queue: &Queue) -> Vec<String> {
    lines_with(queue, &HashMap::new())
}

/// Each row's status line, in sidebar order, finished jobs summed up by `summaries`.
fn lines_with(queue: &Queue, summaries: &HashMap<JobId, RowSummary>) -> Vec<String> {
    lines_fixing(queue, summaries, &HashMap::new())
}

/// As `lines_with`, with Fix It at step `fixing[video]` on each video it fixes.
fn lines_fixing(
    queue: &Queue,
    summaries: &HashMap<JobId, RowSummary>,
    fixing: &HashMap<PathBuf, usize>,
) -> Vec<String> {
    let now = Instant::now();
    sidebar_rows::rows(queue)
        .iter()
        .map(|row| {
            let item = queue.get(row.id).expect("the row's job");
            let rates = time_left::pilot_rates();
            let summary = summaries.get(&row.id);
            let step = fixing.get(&item.video).copied();
            status(row, item, queue, &rates, summary, step, now)
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
        finished: Vec::new(),
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
        finished: Vec::new(),
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
    queue_review(&mut q, PathBuf::from("a"), 1);
    queue_review(&mut q, PathBuf::from("a"), 1);
    assert_eq!(lines(&q), ["Updating subtitles · 2 corrections"]);
}

#[test]
fn finished_rows_give_their_verdict_and_lines_to_check_from_their_files() {
    let mut q = queue(&["a", "b", "c", "d", "e"]);
    for item in q
        .items
        .iter_mut()
        .filter(|item| item.video != Path::new("d"))
    {
        item.state = JobState::FinishedBefore;
    }
    q.items[3].state = JobState::Finished(JobResult {
        subtitles: PathBuf::from("d.srt"),
        work_dir: PathBuf::from("work"),
        failures: Vec::new(),
        findings: 3,
        wall_s: 60.0,
    });
    let summary = |problems, flagged, to_check| RowSummary {
        problems,
        flagged,
        to_check,
        fixed_by_claude: false,
    };
    let ids: Vec<JobId> = q.items.iter().map(|item| item.id).collect();
    let summaries = HashMap::from([
        (ids[0], summary(0, 40, 38)),
        (ids[1], summary(0, 12, 0)),
        (ids[2], summary(2, 40, 38)),
        (ids[3], summary(1, 0, 0)),
        (ids[4], summary(0, 0, 0)),
    ]);
    let mut shown = lines_with(&q, &summaries);
    shown.sort();
    assert_eq!(
        shown,
        [
            "Needs attention · 1 problem",
            "Needs attention · 2 problems",
            "Subtitles ready",
            "Subtitles ready · 38 to check",
            "Subtitles ready · all checked",
        ],
        "a row finished in an earlier window shows its real verdict; with no line worth a \
         listen there is nothing to check"
    );
    for (item, fails) in q.items.iter().zip([false, false, true, true, false]) {
        assert_eq!(fails_the_check(item, summaries.get(&item.id)), fails);
    }
    assert!(!fails_the_check(&q.items[0], None), "unread, it passes");
}

#[test]
fn a_row_fixed_by_claude_says_so_and_a_row_being_fixed_gives_its_step() {
    let mut q = queue(&["a", "b", "c", "d"]);
    for item in &mut q.items {
        item.state = JobState::FinishedBefore;
    }
    let fixed = |problems, to_check| RowSummary {
        problems,
        flagged: 38,
        to_check,
        fixed_by_claude: true,
    };
    let ids: Vec<JobId> = q.items.iter().map(|item| item.id).collect();
    let summaries = HashMap::from([
        (ids[0], fixed(0, 0)),
        (ids[1], fixed(0, 2)),
        (ids[2], fixed(1, 0)),
        (ids[3], fixed(0, 0)),
    ]);
    let fixing = HashMap::from([(PathBuf::from("d"), 2)]);
    assert_eq!(
        lines_fixing(&q, &summaries, &fixing),
        [
            "Subtitles ready · fixed by Claude",
            "Subtitles ready · 2 to check",
            "Needs attention · 1 problem",
            "Fixing with Claude · 2 of 4",
        ],
        "fixed by Claude only while it passes with nothing to check"
    );
    queue_review(&mut q, PathBuf::from("d"), 3);
    let fixing = HashMap::from([(PathBuf::from("d"), 4)]);
    assert_eq!(
        lines_fixing(&q, &summaries, &fixing)[3],
        "Fixing with Claude · 4 of 4",
        "Fix It's correction run is its last step, not a plain update"
    );
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

#[test]
fn the_detail_line_gives_a_running_jobs_length_and_a_waiting_jobs_place() {
    let mut q = queue(&["a", "b", "c"]);
    let now = Instant::now();
    assert_eq!(
        detail_line(&q.items[0], &q, now),
        "Length known once it starts · next in line"
    );
    assert_eq!(
        detail_line(&q.items[2], &q, now),
        "Length known once it starts · Waiting"
    );
    let mut progress = JobProgress::new(now - Duration::from_secs(600));
    q.items[0].state = JobState::Running(Box::new(progress.clone()));
    q.running = true;
    assert_eq!(detail_line(&q.items[0], &q, now), "Running for 10 min 00 s");
    assert_eq!(
        detail_line(&q.items[2], &q, now),
        "Length known once it starts · 2nd in line"
    );
    progress.duration_s = Some(1559.0);
    q.items[0].state = JobState::Running(Box::new(progress));
    assert_eq!(
        detail_line(&q.items[0], &q, now),
        "25:59 video · running for 10 min 00 s"
    );
    q.items[1].state = JobState::Cancelled { kept_steps: 1 };
    assert_eq!(
        detail_line(&q.items[1], &q, now),
        "Cancelled · 1 finished step kept"
    );
}

#[test]
fn a_waiting_job_says_when_it_starts() {
    let mut q = queue(&["a", "b"]);
    assert_eq!(place_in_line(&q, &q.items[1]), 2);
    let (a, b) = (q.items[0].clone(), q.items[1].clone());
    assert_eq!(
        waiting_start(&q, &a, 1, false),
        "Press Start Queue to begin."
    );
    q.running = true;
    assert_eq!(
        waiting_start(&q, &a, 1, false),
        "It starts when the current video finishes."
    );
    assert_eq!(
        waiting_start(&q, &b, 2, false),
        "It starts when the videos before it finish."
    );
    assert_eq!(
        waiting_start(&q, &b, 2, true),
        "It can start once the models are on disk."
    );
    q.running = false;
    let review = queue_review(&mut q, PathBuf::from("a"), 1);
    let review = q.get(review).expect("the correction run").clone();
    assert_eq!(
        waiting_start(&q, &review, 1, false),
        "It starts as soon as the video is free.",
        "a correction run starts on its own, the queue running or not"
    );
}

#[test]
fn a_job_tried_again_starts_at_once_only_when_its_lane_and_video_are_idle() {
    let mut q = queue(&["a", "b"]);
    q.items[0].state = JobState::Cancelled { kept_steps: 2 };
    assert_eq!(
        try_again_start(&q, &q.items[0], false),
        "It starts at once."
    );
    assert_eq!(
        try_again_start(&q, &q.items[0], true),
        "It waits until the models are on disk."
    );
    q.items[1].state = JobState::Running(Box::new(JobProgress::new(Instant::now())));
    assert_eq!(
        try_again_start(&q, &q.items[0], false),
        "It goes first in line and waits for Start Queue."
    );
    q.running = true;
    assert_eq!(
        try_again_start(&q, &q.items[0], false),
        "It runs next, when the current video finishes."
    );
    // The full lane is idle, but a correction run of the same video runs.
    q.running = false;
    q.items[1].state = JobState::FinishedBefore;
    let review = queue_review(&mut q, PathBuf::from("a"), 1);
    if let Some(item) = q.get_mut(review) {
        item.state = JobState::Running(Box::new(JobProgress::new(Instant::now())));
    }
    assert_eq!(
        try_again_start(&q, &q.items[0], false),
        "It goes first in line and waits for Start Queue."
    );
}

#[test]
fn a_correction_run_tried_again_starts_at_once_while_a_review_lane_is_idle() {
    let names: Vec<String> = (0..=REVIEW_LANES).map(|n| format!("v{n}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut q = queue(&names);
    let reviews: Vec<JobId> = names
        .iter()
        .map(|name| queue_review(&mut q, PathBuf::from(name), 1))
        .collect();
    let run = |q: &mut Queue, id| {
        if let Some(item) = q.get_mut(id) {
            item.state = JobState::Running(Box::new(JobProgress::new(Instant::now())));
        }
    };
    if let Some(item) = q.get_mut(reviews[0]) {
        item.state = JobState::Cancelled { kept_steps: 1 };
    }
    for &id in &reviews[1..REVIEW_LANES] {
        run(&mut q, id);
    }
    let cancelled = q.get(reviews[0]).expect("the cancelled run").clone();
    assert_eq!(
        try_again_start(&q, &cancelled, false),
        "It starts at once.",
        "three correction runs leave a lane idle"
    );
    run(&mut q, reviews[REVIEW_LANES]);
    assert_eq!(
        try_again_start(&q, &cancelled, false),
        "It runs next, when the current video finishes.",
        "four fill every lane"
    );
}
