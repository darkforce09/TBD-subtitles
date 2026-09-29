use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Barrier, mpsc};

use super::*;

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(limit: usize) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "tbd-claude-slots-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        write_config(
            &path,
            Config {
                limit,
                slots: limit,
            },
        )
        .unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn try_once(directory: &Path) -> Option<Permit> {
    let registry = registry(directory).unwrap();
    registry.lock().unwrap();
    available_slot(directory, read_config(directory).unwrap()).unwrap()
}

#[test]
fn independent_file_handles_share_one_concurrency_cap() {
    let directory = TestDirectory::new(3);
    let running = AtomicUsize::new(0);
    let peak = AtomicUsize::new(0);
    let start = Barrier::new(12);
    std::thread::scope(|scope| {
        for _ in 0..12 {
            scope.spawn(|| {
                start.wait();
                for _ in 0..3 {
                    let _permit = acquire_at(&directory.0, None).unwrap();
                    let held = running.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(held, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(20));
                    running.fetch_sub(1, Ordering::SeqCst);
                }
            });
        }
    });
    assert!(peak.load(Ordering::SeqCst) <= 3);
    assert!(peak.load(Ordering::SeqCst) > 1);
    assert_eq!(running.load(Ordering::SeqCst), 0);
}

#[test]
fn cancelling_a_wait_leaves_the_running_call_alone() {
    let directory = TestDirectory::new(1);
    let held = acquire_at(&directory.0, None).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let (sent, received) = mpsc::channel();
    std::thread::scope(|scope| {
        let wait_cancel = cancel.clone();
        let path = &directory.0;
        scope.spawn(move || {
            let result = acquire_at(path, Some(&wait_cancel)).map(|_| ());
            sent.send(result).unwrap();
        });
        assert!(matches!(
            received.recv_timeout(Duration::from_millis(150)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        cancel_wait(&cancel, &received);
    });
    assert!(try_once(&directory.0).is_none());
    drop(held);
    assert!(try_once(&directory.0).is_some());
}

fn cancel_wait(cancel: &AtomicBool, received: &mpsc::Receiver<Result<(), LlmError>>) {
    cancel.store(true, Ordering::Relaxed);
    let error = received
        .recv_timeout(Duration::from_secs(2))
        .expect("the cancelled waiter responds")
        .unwrap_err();
    assert_eq!(error.0, "cancelled");
}

#[test]
fn cancellation_also_interrupts_waiting_for_the_registry() {
    let directory = TestDirectory::new(1);
    let registry = registry(&directory.0).unwrap();
    registry.lock().unwrap();
    let cancel = AtomicBool::new(false);
    let (sent, received) = mpsc::channel();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            sent.send(acquire_at(&directory.0, Some(&cancel)).map(|_| ()))
                .unwrap();
        });
        assert!(matches!(
            received.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        cancel_wait(&cancel, &received);
    });
}

#[test]
fn dropping_a_permit_releases_its_slot_for_another_handle() {
    let directory = TestDirectory::new(1);
    let permit = acquire_at(&directory.0, None).unwrap();
    assert!(try_once(&directory.0).is_none());
    drop(permit);
    let replacement = try_once(&directory.0).expect("the OS releases the dropped lock");
    assert!(try_once(&directory.0).is_none());
    drop(replacement);
    assert!(try_once(&directory.0).is_some());
}

#[cfg(unix)]
#[test]
fn a_duplicated_descriptor_does_not_extend_a_completed_calls_permit() {
    let directory = TestDirectory::new(1);
    let permit = acquire_at(&directory.0, None).unwrap();
    // A fork before exec retains the same open-file description, just as try_clone does.
    let inherited = permit._slot.try_clone().unwrap();
    assert!(try_once(&directory.0).is_none());
    drop(permit);
    let replacement = try_once(&directory.0)
        .expect("a completed call releases its slot while an inherited descriptor is still open");
    assert!(try_once(&directory.0).is_none());
    drop(inherited);
    assert!(try_once(&directory.0).is_none());
    drop(replacement);
    assert!(try_once(&directory.0).is_some());
}

#[cfg(unix)]
#[test]
fn duplicated_registry_and_probe_descriptors_do_not_keep_temporary_locks() {
    let directory = TestDirectory::new(1);
    for name in ["registry.lock", "slot-0.lock"] {
        let path = directory.0.join(name);
        let temporary = open_lock(&path).unwrap();
        temporary.lock().unwrap();
        let inherited = temporary.try_clone().unwrap();
        let contender = open_lock(&path).unwrap();
        assert!(matches!(
            contender.try_lock(),
            Err(TryLockError::WouldBlock)
        ));
        drop(temporary);
        contender
            .try_lock()
            .expect("the temporary owner releases its lock independently of inherited descriptors");
        drop(inherited);
        let other = open_lock(&path).unwrap();
        assert!(matches!(other.try_lock(), Err(TryLockError::WouldBlock)));
    }
}

#[test]
fn lowering_the_limit_counts_outstanding_high_numbered_slots() {
    let directory = TestDirectory::new(3);
    let low = acquire_at(&directory.0, None).unwrap();
    let middle = acquire_at(&directory.0, None).unwrap();
    let high = acquire_at(&directory.0, None).unwrap();
    set_limit_at(&directory.0, 1).unwrap();
    drop(low);
    drop(middle);
    assert_eq!(read_config(&directory.0).unwrap().slots, 3);
    assert!(try_once(&directory.0).is_none());
    drop(high);
    assert!(try_once(&directory.0).is_some());
}

#[test]
fn raising_the_limit_admits_a_new_call_without_disturbing_the_first() {
    let directory = TestDirectory::new(1);
    let _first = acquire_at(&directory.0, None).unwrap();
    assert!(try_once(&directory.0).is_none());
    set_limit_at(&directory.0, 2).unwrap();
    let _second = try_once(&directory.0).expect("the higher limit is visible immediately");
    assert!(try_once(&directory.0).is_none());
}

#[test]
fn limits_persist_atomically_and_ignore_an_unfinished_part_file() {
    let directory = TestDirectory::new(2);
    let limit_file = directory.0.join("limit.json");
    let before = File::open(&limit_file).unwrap();
    set_limit_at(&directory.0, 5).unwrap();
    // An open reader keeps the complete old file when the pathname is replaced.
    let old: serde_json::Value = serde_json::from_reader(before).unwrap();
    assert_eq!(old["limit"], 2);
    assert_eq!(read_config(&directory.0).unwrap().limit, 5);
    assert!(!directory.0.join("limit.json.part").exists());

    std::fs::write(directory.0.join("limit.json.part"), b"{unfinished").unwrap();
    assert_eq!(read_config(&directory.0).unwrap().limit, 5);
    set_limit_at(&directory.0, 0).unwrap();
    let config = read_config(&directory.0).unwrap();
    assert_eq!(config.limit, 1);
    assert_eq!(config.slots, 5);
}

#[test]
fn an_invalid_persisted_limit_fails_closed() {
    let directory = TestDirectory::new(1);
    for value in [
        "{unfinished",
        r#"{"limit":0,"slots":1}"#,
        r#"{"limit":2,"slots":1}"#,
        r#"{"limit":1,"slots":"many"}"#,
    ] {
        std::fs::write(directory.0.join("limit.json"), value).unwrap();
        assert!(acquire_at(&directory.0, None).is_err());
    }
}
