use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

use super::*;

fn lock_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("tbd-gpu-lock-{}-{name}", std::process::id()))
}

#[test]
fn a_free_lock_is_taken_without_waiting() {
    let path = lock_path("free");
    let waited = Cell::new(false);
    let lock = acquire(&path, &CancelToken::new(), &|| waited.set(true));
    assert!(lock.is_ok());
    assert!(!waited.get());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_held_lock_waits_until_released() {
    let path = lock_path("held");
    let first = acquire(&path, &CancelToken::new(), &|| {}).expect("first");
    let waited = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let second = scope.spawn(|| {
            acquire(&path, &CancelToken::new(), &|| {
                waited.store(true, Ordering::SeqCst)
            })
        });
        std::thread::sleep(Duration::from_millis(400));
        drop(first);
        assert!(second.join().expect("thread").is_ok());
    });
    assert!(waited.load(Ordering::SeqCst));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_cancelled_wait_gives_up() {
    let path = lock_path("cancel");
    let _first = acquire(&path, &CancelToken::new(), &|| {}).expect("first");
    let cancel = CancelToken::new();
    let error = acquire(&path, &cancel, &|| cancel.cancel()).expect_err("cancelled");
    assert!(error.is_cancelled());
    let _ = std::fs::remove_file(&path);
}
