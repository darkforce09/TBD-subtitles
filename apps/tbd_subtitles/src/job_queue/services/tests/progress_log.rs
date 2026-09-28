use std::path::PathBuf;

use job_model::job::StepMeasure;

use super::*;

fn advanced(log: &mut ProgressLog, done: usize, total: usize) -> Option<(Level, String)> {
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
        Some((Level::DEBUG, "asr_parakeet: 3 of 4".to_string()))
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
fn a_finished_step_names_its_time_memory_and_notes() {
    let mut log = ProgressLog::default();
    let measure = StepMeasure {
        wall_s: 12.34,
        peak_ram_mib: Some(512.4),
        peak_vram_mib: Some(2048.0),
        notes: [("chunks".to_string(), "7".to_string())].into(),
        ..StepMeasure::default()
    };
    let (level, text) = log
        .describe(&Progress::StepFinished {
            step: StepName::AsrParakeet,
            measure,
        })
        .unwrap();
    assert_eq!(level, Level::INFO);
    assert_eq!(
        text,
        "asr_parakeet: finished in 12.3 s, 512 MiB RAM, 2048 MiB VRAM, chunks=7"
    );
}

#[test]
fn a_start_lists_the_steps_to_run_and_a_failure_is_an_error() {
    let mut log = ProgressLog::default();
    let (_, text) = log
        .describe(&Progress::JobStarted {
            video: PathBuf::from("/v/Dressrosa 12.mkv"),
            work_dir: PathBuf::from("/w/abc"),
            stale: vec![StepName::ProbeDecode, StepName::AsrParakeet],
        })
        .unwrap();
    assert_eq!(
        text,
        "started /v/Dressrosa 12.mkv in /w/abc; to run: probe_decode, asr_parakeet"
    );
    let (level, text) = log
        .describe(&Progress::StepFailed {
            step: StepName::AsrParakeet,
            message: "the worker exited 1".to_string(),
        })
        .unwrap();
    assert_eq!(level, Level::ERROR);
    assert_eq!(text, "asr_parakeet: failed: the worker exited 1");
}

#[test]
fn a_stopped_job_ends_with_an_error_line() {
    let error = PipelineError::new("step transcribe", "boom");
    let (level, text) = describe_end(&Err(error));
    assert_eq!(level, Level::ERROR);
    assert!(text.starts_with("stopped: "), "{text}");
    assert!(text.contains("boom"), "{text}");
}
