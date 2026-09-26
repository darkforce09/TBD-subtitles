use std::io::Read;
use std::time::{Duration, Instant};

use crate::{Run, RunError};

#[test]
fn streams_stdout_and_keeps_stderr() {
    let mut running = Run::new("sh")
        .arg("-c")
        .arg("echo one; echo warn >&2; echo two")
        .spawn()
        .unwrap();
    let mut text = String::new();
    running
        .take_stdout()
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    let finished = running.wait().unwrap();
    assert_eq!(text, "one\ntwo\n");
    assert_eq!(finished.stderr, "warn\n");
    assert_eq!(finished.code, 0);
}

#[test]
fn a_deadline_kills_a_reader_blocked_child() {
    let started = Instant::now();
    let mut running = Run::new("sh")
        .arg("-c")
        .arg("sleep 30")
        .timeout(Duration::from_millis(300))
        .spawn()
        .unwrap();
    let mut sink = Vec::new();
    // Blocks until the watchdog kills the group and the pipe closes.
    let _ = running.take_stdout().unwrap().read_to_end(&mut sink);
    let result = running.wait();
    assert!(
        matches!(result, Err(RunError::Timeout { .. })),
        "{result:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn dropping_an_unwaited_handle_kills_the_child() {
    let running = Run::new("sh").arg("-c").arg("sleep 30").spawn().unwrap();
    let pid = running.pid() as i32;
    drop(running);
    // SAFETY: signal 0 only probes whether the pid exists.
    let alive = unsafe { libc::kill(pid, 0) } == 0;
    assert!(!alive, "the dropped child must be gone");
}

#[test]
fn the_raw_exit_code_passes_through() {
    let running = Run::new("sh").arg("-c").arg("exit 7").spawn().unwrap();
    assert_eq!(running.wait().unwrap().code, 7);
}

#[test]
fn a_child_dies_with_the_thread_that_started_it() {
    // `exec` keeps the pid, so the pid we hold is `sleep` itself.
    let pid = std::thread::spawn(|| {
        let running = Run::new("sh")
            .arg("-c")
            .arg("exec sleep 30")
            .spawn()
            .unwrap();
        let pid = running.pid() as i32;
        // Leak the handle: only the thread's end may stop the child.
        std::mem::forget(running);
        pid
    })
    .join()
    .unwrap();
    let dead = (0..100).any(|_| {
        // A killed child stays a zombie until reaped; either way it no longer runs.
        let state = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
        let running = !state.is_empty()
            && state
                .rsplit(')')
                .next()
                .is_some_and(|rest| !rest.trim_start().starts_with('Z'));
        if running {
            std::thread::sleep(Duration::from_millis(20));
        }
        !running
    });
    // SAFETY: reap the zombie so it does not outlive the test; the pid is our own child.
    unsafe { libc::waitpid(pid, std::ptr::null_mut(), 0) };
    assert!(dead, "the child outlived the thread that started it");
}
