use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use job_model::report::QcReport;
use pipeline::workers::Binaries;
use pipeline::{JobOptions, JobOutcome};

use super::*;
use crate::core::background::no_wake;

fn options(cancel: CancelToken) -> JobOptions {
    JobOptions {
        work_root: PathBuf::from("/work"),
        settings: job_model::job::JobSettings::with_glossary(vec![]),
        rerun: Vec::new(),
        binaries: Binaries {
            main: PathBuf::from("/bin/a"),
            ggml: PathBuf::from("/bin/b"),
            local_llm: PathBuf::from("/bin/c"),
        },
        cancel,
        gpu_lock: PathBuf::from("/tmp/gpu.lock"),
        library: None,
        models_dir: PathBuf::from("/models"),
    }
}

/// A stand-in correction run that ends at once.
fn stand_in() -> RunJob {
    Arc::new(|video, _, _| {
        Ok(JobOutcome {
            work_dir: PathBuf::from("/work/a"),
            subtitles: video.with_extension("srt"),
            report: PathBuf::from("/work/a/report.md"),
            qc: QcReport::default(),
            ran: Vec::new(),
            skipped: Vec::new(),
        })
    })
}

fn command(id: JobId, token: &CancelToken) -> Command {
    Command {
        id,
        video: PathBuf::from(format!("/videos/{id}.mkv")),
        options: options(token.clone()),
    }
}

#[test]
fn four_lanes_fill_and_a_fifth_run_is_refused() {
    let mut lanes = ReviewLanes::start(stand_in(), no_wake());
    for id in 1..=REVIEW_LANES as JobId {
        assert!(lanes.free(), "a lane is idle before run {id}");
        let token = CancelToken::new();
        assert_eq!(lanes.run(id, command(id, &token), token), Ok(()));
    }
    assert!(!lanes.free(), "every lane holds a run");
    let token = CancelToken::new();
    assert!(lanes.run(5, command(5, &token), token.clone()).is_err());
    assert!(!lanes.hold(5, token), "nor can one be held");
    assert!(!lanes.is_running(5));
    assert_eq!(lanes.tokens().count(), REVIEW_LANES);
    // Each run ended on its own lane's thread.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut events = Vec::new();
    while events.len() < REVIEW_LANES && Instant::now() < deadline {
        lanes.drain(&mut events);
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut ended: Vec<JobId> = events
        .iter()
        .filter_map(|event| match event {
            RunnerEvent::Ended(id, Ok(_)) => Some(*id),
            _ => None,
        })
        .collect();
    ended.sort_unstable();
    assert_eq!(ended, vec![1, 2, 3, 4]);
    assert!(!lanes.free(), "a lane stays held until its run is released");
}

#[test]
fn releasing_a_run_frees_its_lane() {
    let mut lanes = ReviewLanes::start(stand_in(), no_wake());
    for id in 1..=REVIEW_LANES as JobId {
        assert!(lanes.hold(id, CancelToken::new()));
    }
    assert!(!lanes.free());
    lanes.release(99);
    assert!(!lanes.free(), "releasing a run no lane holds frees nothing");
    lanes.release(2);
    assert!(lanes.free());
    assert!(!lanes.is_running(2));
    assert!(
        lanes.hold(7, CancelToken::new()),
        "the freed lane takes a new run"
    );
    assert!(lanes.is_running(7));
    assert!(!lanes.free());
}

#[test]
fn a_runs_token_is_its_own() {
    let mut lanes = ReviewLanes::start(stand_in(), no_wake());
    let (first, second) = (CancelToken::new(), CancelToken::new());
    assert!(lanes.hold(10, first.clone()));
    assert!(lanes.hold(11, second.clone()));
    lanes.token(11).expect("run 11 is held").cancel();
    assert!(second.is_cancelled(), "the token stops run 11");
    assert!(!first.is_cancelled(), "and leaves run 10 running");
    assert!(lanes.token(12).is_none());
}
