use std::path::PathBuf;

use job_model::job::StepMeasure;

use super::*;

fn advanced(log: &mut ProgressLog, done: usize, total: usize) -> Option<JobLine> {
    log.describe(&Progress::StepAdvanced {
        step: StepName::AsrParakeet,
        done,
        total,
    })
}

#[test]
fn a_step_advance_is_logged_once_per_tenth() {
    let mut log = ProgressLog::default();
    let logged: Vec<usize> = (0..=100)
        .filter(|&done| advanced(&mut log, done, 100).is_some())
        .collect();
    assert_eq!(logged, [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100]);
    assert_eq!(
        advanced(&mut ProgressLog::default(), 3, 4),
        Some(JobLine::new(
            Level::DEBUG,
            Some(StepName::AsrParakeet),
            "3 of 4 done"
        ))
    );
}

#[test]
fn a_restarted_step_logs_its_advance_again_and_an_empty_total_logs_nothing() {
    let mut log = ProgressLog::default();
    assert!(advanced(&mut log, 5, 10).is_some());
    log.describe(&Progress::StepStarted(StepName::AsrParakeet));
    assert!(advanced(&mut log, 5, 10).is_some());
    assert!(advanced(&mut log, 0, 0).is_none());
}

#[test]
fn a_finished_step_names_its_time_memory_and_notes_but_not_itself() {
    let mut log = ProgressLog::default();
    let measure = StepMeasure {
        wall_s: 12.34,
        peak_ram_mib: Some(512.4),
        peak_vram_mib: Some(2048.0),
        notes: [("chunks".to_string(), "7".to_string())].into(),
        ..StepMeasure::default()
    };
    let line = log
        .describe(&Progress::StepFinished {
            step: StepName::AsrParakeet,
            measure,
        })
        .unwrap();
    assert_eq!(line.level, Level::INFO);
    assert_eq!(line.step, Some(StepName::AsrParakeet));
    assert_eq!(
        line.text,
        "Step finished in 12.3 s, 512 MiB RAM, 2048 MiB VRAM, chunks=7"
    );
}

#[test]
fn a_start_lists_the_steps_to_run_and_a_failure_is_an_error() {
    let mut log = ProgressLog::default();
    let line = log
        .describe(&Progress::JobStarted {
            video: PathBuf::from("/v/Dressrosa 12.mkv"),
            work_dir: PathBuf::from("/w/abc"),
            stale: vec![StepName::ProbeDecode, StepName::AsrParakeet],
        })
        .unwrap();
    assert_eq!(line.step, None);
    assert_eq!(
        line.text,
        "Job started for /v/Dressrosa 12.mkv in /w/abc; to run: probe_decode, asr_parakeet"
    );
    let line = log
        .describe(&Progress::StepFailed {
            step: StepName::AsrParakeet,
            message: "the worker exited 1".to_string(),
        })
        .unwrap();
    assert_eq!(line.level, Level::ERROR);
    assert_eq!(line.text, "Step failed: the worker exited 1");
}

#[test]
fn a_model_call_is_no_line_and_a_stopped_job_ends_with_an_error() {
    let mut log = ProgressLog::default();
    let call = Progress::ModelCall {
        step: StepName::Adjudicate,
        call: Box::default(),
    };
    assert_eq!(log.describe(&call), None);
    let error = PipelineError::new("step asr_parakeet", "boom");
    let line = describe_end(&Err(error), None);
    assert_eq!(line.level, Level::ERROR);
    assert!(line.text.starts_with("Job stopped: "), "{}", line.text);
    assert!(line.text.contains("boom"), "{}", line.text);
}

#[test]
fn a_worker_s_model_call_reaches_the_log_window_under_the_job_s_span() {
    use std::sync::Arc;

    use tracing_subscriber::layer::SubscriberExt as _;

    use crate::core::log_buffer::{ConsoleLayer, LogBuffer};

    let buffer = Arc::new(LogBuffer::new());
    let subscriber = tracing_subscriber::registry().with(ConsoleLayer::new(buffer.clone()));
    let call = ModelExchange {
        id: "9-2".into(),
        purpose: "sound cues, window 1 of 3".into(),
        ..ModelExchange::default()
    };
    tracing::subscriber::with_default(subscriber, || {
        let job = tracing::info_span!("job", video = "Dressrosa 12");
        let _job = job.enter();
        let step = tracing::info_span!("step", step = "sound_cues");
        let _step = step.enter();
        emit_call(&call);
    });
    let kept = buffer.calls_since(0);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].call, call);
    assert_eq!(kept[0].video.as_deref(), Some("Dressrosa 12"));
    assert_eq!(kept[0].step.as_deref(), Some("sound_cues"));
    assert!(buffer.since(0).is_empty(), "a call is never a line");
}

#[test]
fn a_finished_job_names_its_subtitles_and_its_localized_video() {
    let outcome = || {
        Ok(JobOutcome {
            work_dir: PathBuf::from("/work/job"),
            subtitles: PathBuf::from("/videos/a.ass"),
            report: PathBuf::from("/work/job/report.md"),
            qc: job_model::report::QcReport::default(),
            ran: vec![StepName::Cues, StepName::Output],
            skipped: vec![StepName::Vad],
        })
    };
    let line = describe_end(&outcome(), None);
    assert_eq!(line.level, Level::INFO);
    assert_eq!(
        line.text,
        "Job finished: 2 steps ran, 1 kept; subtitles /videos/a.ass"
    );
    let localized = PathBuf::from("/videos/a.localized.mkv");
    let line = describe_end(&outcome(), Some(&localized));
    assert_eq!(
        line.text,
        "Job finished: 2 steps ran, 1 kept; subtitles /videos/a.ass; localized video \
         /videos/a.localized.mkv"
    );
}
