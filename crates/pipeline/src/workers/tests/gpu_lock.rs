use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

use super::*;

fn lock_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("tbd-gpu-lock-{}-{name}", std::process::id()))
}

fn holder(step: StepName) -> Holder {
    Holder {
        step,
        job: PathBuf::from("/jobs/a"),
    }
}

#[test]
fn a_free_lock_is_taken_without_waiting() {
    let path = lock_path("free");
    let waited = Cell::new(false);
    let lock = acquire(
        &path,
        holder(StepName::TextDetect),
        &CancelToken::new(),
        &|_| waited.set(true),
    );
    assert!(lock.is_ok());
    assert!(!waited.get());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_held_lock_waits_until_released_and_names_its_holder() {
    let path = lock_path("held");
    let first = acquire(
        &path,
        holder(StepName::TextDetect),
        &CancelToken::new(),
        &|_| {},
    )
    .expect("first");
    let waited = AtomicBool::new(false);
    let named = Mutex::new(None);
    std::thread::scope(|scope| {
        let second = scope.spawn(|| {
            acquire(
                &path,
                holder(StepName::Alignment),
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
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_released_lock_names_no_holder() {
    let path = lock_path("released");
    let first = acquire(
        &path,
        holder(StepName::TextRead),
        &CancelToken::new(),
        &|_| {},
    )
    .expect("first");
    drop(first);
    let held = HELD.lock().unwrap_or_else(PoisonError::into_inner);
    assert!(held.iter().all(|(locked, _)| *locked != path));
    drop(held);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_cancelled_wait_gives_up() {
    let path = lock_path("cancel");
    let _first = acquire(
        &path,
        holder(StepName::TextDetect),
        &CancelToken::new(),
        &|_| {},
    )
    .expect("first");
    let cancel = CancelToken::new();
    let error = acquire(&path, holder(StepName::Alignment), &cancel, &|_| {
        cancel.cancel()
    })
    .expect_err("cancelled");
    assert!(error.is_cancelled());
    let _ = std::fs::remove_file(&path);
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
