use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use super::*;
use crate::core::background::no_wake;

const TICK: Duration = Duration::from_millis(20);
const PATIENCE: Duration = Duration::from_secs(10);

fn folder(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("tbd-folder-watcher-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

fn write(path: &Path) {
    std::fs::write(path, b"video").expect("file");
}

#[test]
fn a_video_written_once_is_found_and_the_window_woken() {
    let dir = folder("once");
    write(&dir.join("a.mkv"));
    let wakes = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&wakes);
    let watcher = start(
        vec![dir.clone()],
        TICK,
        Arc::new(move || {
            counted.fetch_add(1, Ordering::SeqCst);
        }),
    );
    assert_eq!(watcher.folders(), std::slice::from_ref(&dir));
    let found = watcher.found.recv_timeout(PATIENCE).expect("found");
    assert_eq!(found, [dir.join("a.mkv")]);
    let asked = Instant::now();
    while wakes.load(Ordering::SeqCst) == 0 {
        assert!(asked.elapsed() < PATIENCE, "the window is woken");
        std::thread::yield_now();
    }
    assert!(
        watcher.found.recv_timeout(TICK * 5).is_err(),
        "sent once only"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn new_folders_are_watched_at_once() {
    let first = folder("first");
    let second = folder("second");
    write(&second.join("b.mkv"));
    let mut watcher = start(vec![first.clone()], TICK, no_wake());
    watcher.set_folders(std::slice::from_ref(&second));
    assert_eq!(watcher.folders(), std::slice::from_ref(&second));
    let found = watcher.found.recv_timeout(PATIENCE).expect("found");
    assert_eq!(found, [second.join("b.mkv")]);
    let _ = std::fs::remove_dir_all(&first);
    let _ = std::fs::remove_dir_all(&second);
}

#[test]
fn only_different_folders_are_sent_to_the_thread() {
    let (commands, inbox) = channel();
    let (_send, found) = channel();
    let mut watcher = FolderWatcher {
        found,
        folders: vec![PathBuf::from("a")],
        commands,
    };
    watcher.set_folders(&[PathBuf::from("a")]);
    assert!(inbox.try_recv().is_err(), "the same folders send nothing");
    watcher.set_folders(&[PathBuf::from("b")]);
    assert_eq!(inbox.try_recv(), Ok(vec![PathBuf::from("b")]));
    assert_eq!(watcher.folders(), [PathBuf::from("b")]);
}

#[test]
fn an_empty_folder_list_finds_nothing() {
    let watcher = start(Vec::new(), TICK, no_wake());
    assert!(watcher.found.recv_timeout(TICK * 5).is_err());
}

#[test]
fn dropping_the_watcher_ends_its_thread() {
    let dir = folder("drop");
    let watcher = start(vec![dir.clone()], WATCH_INTERVAL, no_wake());
    let FolderWatcher {
        found, commands, ..
    } = watcher;
    drop(commands);
    let started = Instant::now();
    assert_eq!(
        found.recv_timeout(PATIENCE),
        Err(RecvTimeoutError::Disconnected),
        "the thread ends and drops its sender"
    );
    assert!(
        started.elapsed() < WATCH_INTERVAL,
        "without waiting out the interval"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
