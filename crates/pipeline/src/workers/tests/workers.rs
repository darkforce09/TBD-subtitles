use super::*;

#[test]
fn progress_lines_become_advances_and_anything_else_a_message() {
    assert_eq!(
        parse_line(StepName::AsrParakeet, "progress 3 80"),
        Progress::StepAdvanced {
            step: StepName::AsrParakeet,
            done: 3,
            total: 80
        }
    );
    assert_eq!(
        parse_line(StepName::AsrParakeet, "progress 3 80 extra"),
        Progress::StepMessage {
            step: StepName::AsrParakeet,
            text: "progress 3 80 extra".into()
        }
    );
    assert!(matches!(
        parse_line(StepName::Vad, "loading"),
        Progress::StepMessage { .. }
    ));
}

#[test]
fn a_missing_binary_fails_naming_it() {
    let work = WorkDir::new(std::env::temp_dir());
    let error = run_worker(
        Path::new("/nonexistent/tbd-subtitles-ggml"),
        StepName::AsrWhisper,
        &work,
        &[],
        &|_| {},
        &CancelToken::new(),
        &std::env::temp_dir().join("tbd-gpu-lock-missing-binary"),
    )
    .expect_err("missing");
    assert!(error.message.contains("is missing"), "{error}");
}
