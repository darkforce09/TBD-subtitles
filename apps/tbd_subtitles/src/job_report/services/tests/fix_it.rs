use std::time::{Duration, Instant};

use job_model::outputs::FixRecord;
use pipeline::fix_it::FixStage;

use super::*;
use crate::core::background::no_wake;

fn options() -> FixOptions {
    FixOptions {
        work_root: PathBuf::from("/work"),
        model: "opus".into(),
        glossary_name: "one_piece".into(),
        processes: 2,
        calls: inference::llm::call_gate::CallGate::new(2).seat(),
        cancel: CancelToken::new(),
    }
}

/// Poll `fixing` until its run ends, for at most five seconds.
fn wait(fixing: &mut Fixing) -> Result<FixOutcome, PipelineError> {
    let started = Instant::now();
    loop {
        if let Some(outcome) = fixing.poll() {
            return outcome;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the run never ended"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn the_progress_and_the_outcome_come_back() {
    let run: FixVideo = Arc::new(|video, options, progress| {
        progress(FixProgress {
            stage: FixStage::Reading,
            done: 1,
            total: 1,
        });
        Ok(FixOutcome {
            work_dir: video.to_path_buf(),
            record: FixRecord {
                model: options.model.clone(),
                ..FixRecord::default()
            },
            changed: vec!["U0295".into()],
            kept_yours: Vec::new(),
        })
    });
    let mut fixing = start(
        run,
        "/v/a.mp4".into(),
        options(),
        "Claude Opus".into(),
        no_wake(),
    );
    let outcome = wait(&mut fixing).expect("outcome");
    assert_eq!(outcome.changed, ["U0295"]);
    assert_eq!(outcome.record.model, "opus");
    assert_eq!(fixing.progress.map(|p| p.stage), Some(FixStage::Reading));
}

#[test]
fn stop_ends_the_run_as_cancelled() {
    let run: FixVideo = Arc::new(|_, options, _| {
        while !options.cancel.is_cancelled() {
            std::thread::sleep(Duration::from_millis(2));
        }
        Err(PipelineError::cancelled("Fix It"))
    });
    let mut fixing = start(
        run,
        "/v/a.mp4".into(),
        options(),
        "Claude Opus".into(),
        no_wake(),
    );
    assert!(fixing.poll().is_none());
    fixing.stop();
    assert!(fixing.stopping);
    assert!(wait(&mut fixing).unwrap_err().is_cancelled());
}
