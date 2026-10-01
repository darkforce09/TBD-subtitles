use std::cell::RefCell;
use std::os::fd::AsRawFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::*;

fn lock_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("tbd-lazy-gpu-{}-{name}", std::process::id()))
}

fn remove(path: &Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(gpu_lock::audio_waiting_path(path));
}

/// Whether some descriptor holds the `flock` on `path`.
fn is_locked(path: &Path) -> bool {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .expect("open");
    // SAFETY: `flock` on a descriptor this test owns.
    let taken = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0;
    if taken {
        // SAFETY: as above.
        unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
    }
    !taken
}

/// Plenty of free memory, or the readings given, the last one repeated.
struct Memory(Vec<u64>);

impl FreeMemory for Memory {
    fn free_mib(&mut self) -> Option<u64> {
        if self.0.len() > 1 {
            Some(self.0.remove(0))
        } else {
            self.0.first().copied()
        }
    }
}

/// A model that answers with its name and notes, as it drops, whether the GPU lock was held.
struct FakeModel {
    lock: Option<PathBuf>,
    locked_at_drop: Arc<AtomicBool>,
}

impl LanguageModel for FakeModel {
    fn name(&self) -> String {
        "fake".to_string()
    }

    fn complete_json(
        &mut self,
        _system: &str,
        user: &str,
        _schema: &serde_json::Value,
    ) -> std::result::Result<Completion, LlmError> {
        Ok(Completion {
            json: serde_json::json!({ "echo": user }),
            input_tokens: 1,
            output_tokens: 2,
            cost_usd: None,
        })
    }
}

impl Drop for FakeModel {
    fn drop(&mut self) {
        if let Some(path) = &self.lock {
            self.locked_at_drop.store(is_locked(path), Ordering::SeqCst);
        }
    }
}

fn fake(lock: Option<&Path>, locked_at_drop: &Arc<AtomicBool>) -> FakeModel {
    FakeModel {
        lock: lock.map(Path::to_path_buf),
        locked_at_drop: locked_at_drop.clone(),
    }
}

#[test]
fn only_a_lazily_locking_step_gets_the_lock_variable() {
    let path = Path::new("/data/tbd-subtitles/gpu.lock");
    assert_eq!(
        worker_variable(StepName::TextTranslate, path),
        Some((
            "TBD_SUBTITLES_GPU_LOCK".to_string(),
            "/data/tbd-subtitles/gpu.lock".to_string()
        ))
    );
    assert_eq!(worker_variable(StepName::Alignment, path), None);
    assert_eq!(worker_variable(StepName::TextDetect, path), None);
}

#[test]
fn an_unset_or_empty_variable_names_no_lock() {
    assert_eq!(named_lock(None), None);
    assert_eq!(named_lock(Some(OsString::new())), None);
    assert_eq!(
        named_lock(Some(OsString::from("/data/gpu.lock"))),
        Some(PathBuf::from("/data/gpu.lock"))
    );
}

#[test]
fn without_a_lock_named_the_model_opens_at_once() {
    let dropped = Arc::new(AtomicBool::new(false));
    let model = open_held_with(
        None,
        StepName::TextTranslate,
        Path::new("/jobs/a"),
        &mut Memory(vec![0]),
        &|line| panic!("no wait expected: {line}"),
        || Ok::<_, LlmError>(fake(None, &dropped)),
    )
    .expect("opened");
    assert!(!model.holds_gpu());
}

#[test]
fn the_model_opens_under_the_lock_and_drops_before_it_is_released() {
    let path = lock_path("held");
    let locked_at_drop = Arc::new(AtomicBool::new(false));
    let mut model = open_held_with(
        Some(&path),
        StepName::TextTranslate,
        Path::new("/jobs/a"),
        &mut Memory(vec![8_000]),
        &|line| panic!("no wait expected: {line}"),
        || {
            assert!(is_locked(&path), "the lock is held before the model opens");
            Ok::<_, LlmError>(fake(Some(&path), &locked_at_drop))
        },
    )
    .expect("opened");
    assert!(model.holds_gpu());
    assert_eq!(model.name(), "fake");
    let answer = model
        .complete_json("system", "hello", &serde_json::json!({}))
        .expect("answer");
    assert_eq!(answer.json, serde_json::json!({ "echo": "hello" }));
    assert!(is_locked(&path));
    drop(model);
    assert!(
        locked_at_drop.load(Ordering::SeqCst),
        "the model drops first"
    );
    assert!(!is_locked(&path), "the lock goes with the model");
    remove(&path);
}

#[test]
fn a_model_that_fails_to_open_releases_the_lock() {
    let path = lock_path("failed");
    let error = open_held_with(
        Some(&path),
        StepName::TextTranslate,
        Path::new("/jobs/a"),
        &mut Memory(vec![8_000]),
        &|_| {},
        || Err::<FakeModel, _>(LlmError("no model file".to_string())),
    )
    .expect_err("failed");
    assert_eq!(error.to_string(), "no model file");
    assert!(!is_locked(&path));
    remove(&path);
}

#[test]
fn the_model_waits_for_its_memory_saying_so() {
    let path = lock_path("memory");
    let said = RefCell::new(Vec::new());
    let dropped = Arc::new(AtomicBool::new(false));
    let need = graph::vram_need_mib(StepName::TextTranslate).expect("a need");
    let model = open_held_with(
        Some(&path),
        StepName::TextTranslate,
        Path::new("/jobs/a"),
        &mut Memory(vec![1_000, need]),
        &|line| said.borrow_mut().push(line),
        || Ok::<_, LlmError>(fake(None, &dropped)),
    )
    .expect("opened");
    assert!(model.holds_gpu());
    assert_eq!(
        said.into_inner(),
        [format!(
            "waiting for GPU memory: 1000 MiB free, {need} needed"
        )]
    );
    drop(model);
    remove(&path);
}

#[test]
fn a_worker_names_another_step_when_no_holder_of_its_own_has_the_gpu() {
    let job = Path::new("/jobs/a");
    assert_eq!(
        waiting_line(None, job),
        "waiting for the GPU: another step is using it"
    );
    let holder = Holder {
        step: StepName::TextRead,
        job: job.to_path_buf(),
    };
    assert_eq!(
        waiting_line(Some(&holder), job),
        "waiting for the GPU: text_read of this job is using it"
    );
}

#[test]
fn a_held_model_is_what_the_translation_opener_hands_back() {
    use stages::onscreen_text::translate::LocalOpener;
    let dropped = Arc::new(AtomicBool::new(false));
    let opened = std::cell::Cell::new(false);
    let mut open_local = || -> stages::onscreen_text::TextResult<Box<dyn LanguageModel>> {
        let local = open_held(StepName::TextTranslate, Path::new("/jobs/a"), || {
            opened.set(true);
            Ok::<_, LlmError>(fake(None, &dropped))
        })?;
        Ok(Box::new(local))
    };
    let opener: &mut LocalOpener<'_> = &mut open_local;
    let model = opener().expect("opened");
    assert!(opened.get());
    assert_eq!(model.name(), "fake");
}
