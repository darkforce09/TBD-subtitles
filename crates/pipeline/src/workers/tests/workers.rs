use super::*;

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
