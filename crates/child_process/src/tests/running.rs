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

#[test]
fn a_cancel_flag_kills_a_reader_blocked_child() {
    let started = Instant::now();
    let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut running = Run::new("sh")
        .arg("-c")
        .arg("sleep 30")
        .cancel_on(flag.clone())
        .spawn()
        .unwrap();
    let setter = {
        let flag = flag.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        })
    };
    let mut sink = Vec::new();
    let _ = running.take_stdout().unwrap().read_to_end(&mut sink);
    let result = running.wait();
    setter.join().unwrap();
    assert!(
        matches!(result, Err(RunError::Cancelled { .. })),
        "{result:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn an_unset_cancel_flag_lets_the_child_finish() {
    let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let running = Run::new("sh")
        .arg("-c")
        .arg("exit 3")
        .cancel_on(flag)
        .spawn()
        .unwrap();
    assert_eq!(running.wait().unwrap().code, 3);
}

#[test]
fn a_piped_stdin_streams_bytes_through_the_child() {
    use std::io::Write;
    let mut running = Run::new("cat")
        .stdin_piped()
        .timeout(Duration::from_secs(30))
        .spawn()
        .unwrap();
    let mut stdin = running.take_stdin().unwrap();
    assert!(
        running.take_stdin().is_none(),
        "the stdin is handed out once"
    );
    let stdout = running.take_stdout().unwrap();
    // Read on a thread, so the child never blocks on a full stdout while this one writes.
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut stdout = stdout;
        stdout.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let chunk: Vec<u8> = (0..=255u8).collect();
    for _ in 0..1024 {
        stdin.write_all(&chunk).unwrap();
    }
    drop(stdin);
    let bytes = reader.join().unwrap();
    let finished = running.wait().unwrap();
    assert_eq!(finished.code, 0);
    assert_eq!(bytes.len(), 256 * 1024);
    assert!(bytes.chunks(256).all(|part| part == chunk.as_slice()));
}

#[test]
fn an_untaken_piped_stdin_is_closed_by_the_wait() {
    let running = Run::new("cat")
        .stdin_piped()
        .timeout(Duration::from_secs(10))
        .spawn()
        .unwrap();
    assert_eq!(running.wait().unwrap().code, 0);
}

#[test]
fn a_run_without_a_piped_stdin_hands_none_out() {
    let mut running = Run::new("sh").arg("-c").arg("exit 0").spawn().unwrap();
    assert!(running.take_stdin().is_none());
    assert_eq!(running.wait().unwrap().code, 0);
}

#[test]
fn a_cancel_flag_kills_a_child_while_the_caller_writes() {
    use std::io::Write;
    let started = Instant::now();
    let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    // The child never reads its stdin, so the writer blocks once the pipe buffer is full.
    let mut running = Run::new("sh")
        .arg("-c")
        .arg("sleep 30")
        .stdin_piped()
        .cancel_on(flag.clone())
        .spawn()
        .unwrap();
    let mut stdin = running.take_stdin().unwrap();
    let setter = {
        let flag = flag.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        })
    };
    let chunk = vec![0u8; 64 * 1024];
    let written = (0..1024).try_for_each(|_| stdin.write_all(&chunk));
    assert!(
        written.is_err(),
        "the write must fail once the child is killed"
    );
    drop(stdin);
    let result = running.wait();
    setter.join().unwrap();
    assert!(
        matches!(result, Err(RunError::Cancelled { .. })),
        "{result:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn a_child_killed_by_its_caller_leaves_its_stderr() {
    let started = Instant::now();
    let mut running = Run::new("sh")
        .arg("-c")
        .arg("echo before >&2; echo ready; sleep 30")
        .spawn()
        .unwrap();
    let mut first = [0u8; 6];
    running
        .take_stdout()
        .unwrap()
        .read_exact(&mut first)
        .unwrap();
    assert_eq!(&first, b"ready\n");
    let stderr = running.kill_and_wait().unwrap();
    assert_eq!(stderr, "before\n");
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn a_cancelled_child_stays_cancelled_when_its_caller_kills_it() {
    let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let mut running = Run::new("sh")
        .arg("-c")
        .arg("sleep 30")
        .cancel_on(flag)
        .spawn()
        .unwrap();
    let mut sink = Vec::new();
    let _ = running.take_stdout().unwrap().read_to_end(&mut sink);
    let result = running.kill_and_wait();
    assert!(
        matches!(result, Err(RunError::Cancelled { .. })),
        "{result:?}"
    );
}
