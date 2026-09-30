use super::*;

#[test]
fn a_missing_binary_fails_naming_it() {
    let dir = std::env::temp_dir().join(format!("tbd-worker-missing-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = JobStore::open(&crate::work_dir::WorkDir::new(dir.clone())).expect("store");
    let data = WorkerData {
        store: &store,
        inputs: &[],
    };
    let error = run_worker(
        Path::new("/nonexistent/tbd-subtitles-ggml"),
        StepName::AsrWhisper,
        data,
        &[],
        &|_| {},
        &CancelToken::new(),
        &std::env::temp_dir().join("tbd-gpu-lock-missing-binary"),
    )
    .expect_err("missing");
    assert!(error.message.contains("is missing"), "{error}");
    drop(store);
    let _ = std::fs::remove_dir_all(&dir);
}
