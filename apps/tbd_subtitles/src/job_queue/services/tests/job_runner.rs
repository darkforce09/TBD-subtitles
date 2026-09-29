use std::time::Duration;

use job_model::StepName;
use job_model::report::QcReport;
use pipeline::CancelToken;
use pipeline::workers::Binaries;

use super::*;

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
        models_dir: PathBuf::from("/models"),
    }
}

/// A stand-in job: one step, then success, or cancelled when its token is set.
fn stand_in() -> RunJob {
    Arc::new(|video, options, progress| {
        progress(Progress::StepStarted(StepName::Cues));
        if options.cancel.is_cancelled() {
            return Err(PipelineError::cancelled("step cues"));
        }
        Ok(JobOutcome {
            work_dir: PathBuf::from("/work/a"),
            subtitles: video.with_extension("srt"),
            report: PathBuf::from("/work/a/report.md"),
            qc: QcReport::default(),
            ran: vec![StepName::Cues],
            skipped: Vec::new(),
        })
    })
}

fn next(runner: &JobRunner) -> RunnerEvent {
    runner
        .events
        .recv_timeout(Duration::from_secs(5))
        .expect("an event")
}

#[test]
fn jobs_run_in_order_and_report_each_event() {
    let runner = start("job-runner", stand_in(), crate::core::background::no_wake());
    for id in [4, 5] {
        runner
            .run(Command {
                id,
                video: PathBuf::from(format!("{id}.mp4")),
                options: options(CancelToken::new()),
            })
            .expect("run");
    }
    for id in [4, 5] {
        assert!(matches!(next(&runner), RunnerEvent::Progress(i, _) if i == id));
        match next(&runner) {
            RunnerEvent::Ended(i, Ok(outcome)) => {
                assert_eq!(i, id);
                assert_eq!(outcome.subtitles, PathBuf::from(format!("{id}.srt")));
            }
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn a_cancelled_job_ends_cancelled() {
    let runner = start("job-runner", stand_in(), crate::core::background::no_wake());
    let cancel = CancelToken::new();
    cancel.cancel();
    runner
        .run(Command {
            id: 1,
            video: PathBuf::from("a.mp4"),
            options: options(cancel),
        })
        .expect("run");
    let _ = next(&runner);
    assert!(matches!(next(&runner), RunnerEvent::Ended(1, Err(e)) if e.is_cancelled()));
}
