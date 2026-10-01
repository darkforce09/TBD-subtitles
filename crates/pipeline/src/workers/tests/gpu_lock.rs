use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

use super::*;

fn lock_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("tbd-gpu-lock-{}-{name}", std::process::id()))
}

fn remove(path: &Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(audio_waiting_path(path));
}

fn holder(step: StepName) -> Holder {
    Holder {
        step,
        job: PathBuf::from("/jobs/a"),
    }
}

/// Take the lock at `path` for `step` with the priority the graph gives it.
fn take(path: &Path, step: StepName, cancel: &CancelToken) -> Result<GpuLock> {
    acquire(
        path,
        holder(step),
        crate::graph::gpu_priority(step),
        cancel,
        &|_| {},
    )
}

#[test]
fn a_free_lock_is_taken_without_waiting() {
    let path = lock_path("free");
    let waited = Cell::new(false);
    let lock = acquire(
        &path,
        holder(StepName::TextDetect),
        GpuPriority::Visual,
        &CancelToken::new(),
        &|_| waited.set(true),
    );
    assert!(lock.is_ok());
    assert!(!waited.get());
    drop(lock);
    remove(&path);
}

#[test]
fn a_held_lock_waits_until_released_and_names_its_holder() {
    let path = lock_path("held");
    let first = take(&path, StepName::TextDetect, &CancelToken::new()).expect("first");
    let waited = AtomicBool::new(false);
    let named = Mutex::new(None);
    std::thread::scope(|scope| {
        let second = scope.spawn(|| {
            acquire(
                &path,
                holder(StepName::Alignment),
                GpuPriority::Audio,
                &CancelToken::new(),
                &|held| {
                    *named.lock().expect("named") = held.cloned();
                    waited.store(true, Ordering::SeqCst)
                },
            )
        });
        std::thread::sleep(Duration::from_millis(400));
        drop(first);
        assert!(second.join().expect("thread").is_ok());
    });
    assert!(waited.load(Ordering::SeqCst));
    assert_eq!(
        named.into_inner().expect("named"),
        Some(holder(StepName::TextDetect))
    );
    remove(&path);
}

#[test]
fn a_released_lock_names_no_holder() {
    let path = lock_path("released");
    let first = take(&path, StepName::TextRead, &CancelToken::new()).expect("first");
    drop(first);
    let held = HELD.lock().unwrap_or_else(PoisonError::into_inner);
    assert!(held.iter().all(|(locked, _)| *locked != path));
    drop(held);
    remove(&path);
}

#[test]
fn a_cancelled_wait_gives_up() {
    let path = lock_path("cancel");
    let _first = take(&path, StepName::TextDetect, &CancelToken::new()).expect("first");
    let cancel = CancelToken::new();
    let error = acquire(
        &path,
        holder(StepName::Alignment),
        GpuPriority::Audio,
        &cancel,
        &|_| cancel.cancel(),
    )
    .expect_err("cancelled");
    assert!(error.is_cancelled());
    assert!(
        !audio_waits(&audio_waiting_path(&path)).expect("probe"),
        "a cancelled audio waiter leaves no mark"
    );
    remove(&path);
}

#[test]
fn an_audio_waiter_goes_before_a_visual_one_that_waited_longer() {
    let path = lock_path("priority");
    let first = take(&path, StepName::Separation, &CancelToken::new()).expect("first");
    let order = Mutex::new(Vec::new());
    let wait_for = |step: StepName| {
        let lock = take(&path, step, &CancelToken::new()).expect("taken");
        order.lock().expect("order").push(step);
        std::thread::sleep(Duration::from_millis(300));
        drop(lock);
    };
    let wait_for = &wait_for;
    std::thread::scope(|scope| {
        let visual = scope.spawn(move || wait_for(StepName::TextRead));
        std::thread::sleep(Duration::from_millis(400));
        let audio = scope.spawn(move || wait_for(StepName::Alignment));
        std::thread::sleep(Duration::from_millis(400));
        drop(first);
        audio.join().expect("audio");
        visual.join().expect("visual");
    });
    assert_eq!(
        order.into_inner().expect("order"),
        [StepName::Alignment, StepName::TextRead]
    );
    remove(&path);
}

#[test]
fn a_visual_waiter_takes_the_lock_once_no_audio_waiter_is_left() {
    let path = lock_path("yield");
    let audio_waiting = audio_waiting_path(&path);
    let mark = mark_waiting(&audio_waiting).expect("mark");
    assert!(audio_waits(&audio_waiting).expect("probe"));
    let cancel = CancelToken::new();
    std::thread::scope(|scope| {
        let visual = scope.spawn(|| take(&path, StepName::TextDetect, &cancel));
        std::thread::sleep(Duration::from_millis(400));
        assert!(
            !visual.is_finished(),
            "a free lock waits for the audio waiter"
        );
        drop(mark);
        assert!(visual.join().expect("visual").is_ok());
    });
    remove(&path);
}

#[test]
fn the_waiting_line_names_a_holder_of_this_process() {
    let job = Path::new("/jobs/a");
    assert_eq!(
        waiting_message(Some(&holder(StepName::TextDetect)), job),
        "waiting for the GPU: text_detect of this job is using it"
    );
    let other = Holder {
        step: StepName::Separation,
        job: PathBuf::from("/jobs/b"),
    };
    assert_eq!(
        waiting_message(Some(&other), job),
        "waiting for the GPU: separation of another job is using it"
    );
    assert_eq!(
        waiting_message(None, job),
        "waiting for the GPU: another run of the app is using it"
    );
}
