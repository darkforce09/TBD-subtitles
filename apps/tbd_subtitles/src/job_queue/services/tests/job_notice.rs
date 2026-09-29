use std::path::PathBuf;
use std::time::Instant;

use job_model::StepName;

use super::*;
use crate::job_queue::models::progress::JobProgress;
use crate::job_queue::models::queue::Failure;

fn item(kind: JobKind, state: JobState) -> QueueItem {
    QueueItem {
        id: 1,
        video: PathBuf::from("/videos/[Muhn Pace] Dressrosa 16.mkv"),
        kind,
        state,
        keep_settings: true,
        rerun: Vec::new(),
        corrections: 0,
    }
}

fn finished(failures: &[&str], findings: usize) -> JobState {
    JobState::Finished(JobResult {
        subtitles: PathBuf::from("/videos/[Muhn Pace] Dressrosa 16.srt"),
        work_dir: PathBuf::from("/work/16"),
        failures: failures.iter().map(|f| f.to_string()).collect(),
        findings,
        wall_s: 600.0,
    })
}

fn failed(step: Option<StepName>, message: &str) -> JobState {
    JobState::Failed(Failure::new(step, message.to_string(), Vec::new()))
}

#[test]
fn a_job_that_passes_says_its_subtitles_are_ready() {
    let notice = ended_notice(&item(JobKind::Full, finished(&[], 38))).expect("notice");
    assert_eq!(notice.title, "Subtitles ready: [Muhn Pace] Dressrosa 16");
    assert_eq!(notice.body, "The quality check passed.");
}

#[test]
fn a_job_with_problems_names_them_and_its_lines_to_check() {
    let state = finished(&["2 layout rule(s) broken", "1 failed call(s)"], 12);
    let notice = ended_notice(&item(JobKind::Full, state)).expect("notice");
    assert_eq!(notice.title, "Subtitles ready: [Muhn Pace] Dressrosa 16");
    assert_eq!(
        notice.body,
        "Quality check: 2 layout rule(s) broken, 1 failed call(s); 12 lines to check."
    );
    let one = finished(&["2 layout rule(s) broken"], 1);
    assert_eq!(
        ended_notice(&item(JobKind::Full, one))
            .expect("notice")
            .body,
        "Quality check: 2 layout rule(s) broken; 1 line to check."
    );
}

#[test]
fn a_job_with_problems_and_no_lines_to_check_leaves_the_lines_out() {
    let state = finished(&["2 layout rule(s) broken"], 0);
    let notice = ended_notice(&item(JobKind::Full, state)).expect("notice");
    assert_eq!(notice.body, "Quality check: 2 layout rule(s) broken.");
}

#[test]
fn a_failed_job_says_where_it_failed_and_why() {
    let state = failed(Some(StepName::AsrWhisper), "the worker exited with code 1");
    let notice = ended_notice(&item(JobKind::Full, state)).expect("notice");
    assert_eq!(notice.title, "[Muhn Pace] Dressrosa 16 failed");
    assert_eq!(
        notice.body,
        "At Listen with Whisper: the worker exited with code 1"
    );
    let before = failed(None, "the video cannot be read");
    assert_eq!(
        ended_notice(&item(JobKind::Full, before))
            .expect("notice")
            .body,
        "the video cannot be read"
    );
}

#[test]
fn a_long_failure_is_cut_to_two_hundred_characters() {
    let state = failed(Some(StepName::Qc), &"x".repeat(500));
    let body = ended_notice(&item(JobKind::Full, state))
        .expect("notice")
        .body;
    assert_eq!(body.chars().count(), 200);
    assert!(body.starts_with("At Quality check: xxx"));
    assert!(body.ends_with('…'));
}

#[test]
fn other_jobs_give_no_notice() {
    for state in [
        JobState::Waiting,
        JobState::Running(Box::new(JobProgress::new(Instant::now()))),
        JobState::FinishedBefore,
        JobState::Cancelled { kept_steps: 3 },
    ] {
        assert_eq!(ended_notice(&item(JobKind::Full, state.clone())), None);
    }
    assert_eq!(ended_notice(&item(JobKind::Review, finished(&[], 0))), None);
    assert_eq!(
        ended_notice(&item(JobKind::Review, failed(None, "stopped"))),
        None
    );
}
