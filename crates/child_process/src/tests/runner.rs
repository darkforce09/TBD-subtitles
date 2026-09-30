use std::time::{Duration, Instant};

use super::*;

#[test]
fn captures_stdout_and_raw_code() {
    let out = Run::new("sh")
        .arg("-c")
        .arg("echo hello; exit 3")
        .output()
        .unwrap();
    assert_eq!(out.stdout.trim(), "hello");
    assert_eq!(out.code, 3, "raw exit codes must never be collapsed");
}

#[test]
fn merged_output_preserves_interleaving() {
    // THE POINT. `output()` would give ("a\nc\n", "b\n") and any join invents an order the
    // child never produced. One shared pipe keeps a-b-c.
    let m = Run::new("sh")
        .arg("-c")
        .arg("echo a; echo b >&2; echo c")
        .merged_output()
        .unwrap();
    assert_eq!(m.text, "a\nb\nc\n");
    assert_eq!(m.code, 0);
}

#[test]
fn merged_output_keeps_the_raw_exit_code() {
    let m = Run::new("sh")
        .arg("-c")
        .arg("echo x >&2; exit 3")
        .merged_output()
        .unwrap();
    assert_eq!(m.code, 3);
    assert_eq!(m.text, "x\n");
}

#[test]
fn merged_output_reports_absent_tools_and_signals_honestly() {
    assert!(matches!(
        Run::new("tbd-not-real-3d91").merged_output(),
        Err(RunError::ProgramAbsent(_))
    ));
    assert!(matches!(
        Run::new("sh").arg("-c").arg("kill -9 $$").merged_output(),
        Err(RunError::Signalled { signal: 9, .. })
    ));
}

#[test]
fn merged_output_times_out_without_deadlocking_on_a_full_pipe() {
    // A shared pipe still has a finite buffer; the reader thread must keep draining or the
    // timeout could never fire.
    let got = Run::new("sh")
        .arg("-c")
        .arg("seq 1 200000; sleep 30")
        .timeout(Duration::from_millis(300))
        .merged_output();
    assert!(matches!(got, Err(RunError::Timeout { .. })));
}

#[test]
fn merged_output_honours_cwd_env_and_stdin() {
    let m = Run::new("sh")
        .arg("-c")
        .arg("pwd; echo $TBD_M; cat")
        .cwd("/tmp")
        .env("TBD_M", "set")
        .stdin("fed")
        .merged_output()
        .unwrap();
    assert!(m.text.contains("/tmp"));
    assert!(m.text.contains("set"));
    assert!(m.text.contains("fed"));
}

#[test]
fn captures_stderr_separately() {
    let out = Run::new("sh")
        .arg("-c")
        .arg("echo oops >&2")
        .output()
        .unwrap();
    assert_eq!(out.stderr.trim(), "oops");
    assert!(out.stdout.is_empty());
}

#[test]
fn absent_program_is_program_absent_not_a_failure() {
    let got = Run::new("tbd-definitely-not-a-real-program-9f3a").status();
    assert!(matches!(got, Err(RunError::ProgramAbsent(_))));
}

#[test]
fn signal_death_is_signalled_not_an_exit_code() {
    // THE CONTRACT. A caller that synthesises 128+n reports 137 here, and the next `match` arm
    // reads it as an ordinary failure.
    let got = Run::new("sh").arg("-c").arg("kill -9 $$").status();
    match got {
        Err(RunError::Signalled { signal, .. }) => assert_eq!(signal, 9),
        other => panic!("expected Signalled, got {other:?}"),
    }
}

#[test]
fn timeout_reports_timeout() {
    let got = Run::new("sleep")
        .arg("30")
        .timeout(Duration::from_millis(150))
        .status();
    assert!(matches!(got, Err(RunError::Timeout { .. })));
}

#[test]
fn timeout_kills_the_whole_process_group() {
    // The grandchild outlives its parent unless the GROUP is killed. Marker file proves it:
    // if the tree survived, the sleep completes and writes.
    let marker = std::env::temp_dir().join(format!("child-process-group-{}", std::process::id()));
    let _ = std::fs::remove_file(&marker);
    let script = format!("( sleep 2; touch {} ) & sleep 30", marker.display());
    let got = Run::new("sh")
        .arg("-c")
        .arg(&script)
        .timeout(Duration::from_millis(200))
        .status();
    assert!(matches!(got, Err(RunError::Timeout { .. })));
    std::thread::sleep(Duration::from_millis(2500));
    assert!(
        !marker.exists(),
        "grandchild survived the timeout — the group was not killed"
    );
}

#[test]
fn large_output_does_not_deadlock() {
    // Well past a 64 KiB pipe buffer on both streams at once.
    let out = Run::new("sh")
        .arg("-c")
        .arg("seq 1 60000; seq 1 60000 >&2")
        .timeout(Duration::from_secs(30))
        .output()
        .unwrap();
    assert_eq!(out.code, 0);
    assert!(out.stdout.lines().count() == 60000);
    assert!(out.stderr.lines().count() == 60000);
}

#[test]
fn stdin_is_delivered() {
    let out = Run::new("cat").stdin("piped body").output().unwrap();
    assert_eq!(out.stdout, "piped body");
}

#[test]
fn a_collecting_run_closes_a_piped_stdin() {
    let out = Run::new("cat")
        .stdin_piped()
        .timeout(Duration::from_secs(10))
        .output()
        .unwrap();
    assert_eq!((out.code, out.stdout.as_str()), (0, ""));
}

#[test]
fn the_last_stdin_choice_wins() {
    let out = Run::new("cat")
        .stdin_piped()
        .stdin("body")
        .timeout(Duration::from_secs(10))
        .output()
        .unwrap();
    assert_eq!(out.stdout, "body");
}

#[test]
fn env_and_cwd_apply() {
    let out = Run::new("sh")
        .arg("-c")
        .arg("echo $TBD_X; pwd")
        .cwd("/tmp")
        .env("TBD_X", "set")
        .output()
        .unwrap();
    assert!(out.stdout.contains("set"));
    assert!(out.stdout.contains("/tmp"));
}

#[test]
fn which_finds_and_misses() {
    assert!(which("sh").is_ok());
    assert!(matches!(
        which("tbd-not-real-4b77"),
        Err(RunError::ProgramAbsent(_))
    ));
}

#[test]
fn retry_gives_up_and_returns_the_last_error() {
    let mut n = 0;
    let got: Result<(), RunError> = retry(3, Duration::from_millis(1), || {
        n += 1;
        Err(RunError::Timeout {
            program: "t".into(),
            secs: 0,
        })
    });
    assert!(got.is_err());
    assert_eq!(n, 3);
}

#[test]
fn retry_succeeds_on_a_later_attempt() {
    let mut n = 0;
    let got = retry(5, Duration::from_millis(1), || {
        n += 1;
        if n < 3 {
            Err(RunError::Timeout {
                program: "t".into(),
                secs: 0,
            })
        } else {
            Ok(n)
        }
    });
    assert_eq!(got.unwrap(), 3);
}

#[test]
fn retry_does_not_retry_an_absent_program() {
    let mut n = 0;
    let got: Result<(), RunError> = retry(5, Duration::from_millis(1), || {
        n += 1;
        Err(RunError::ProgramAbsent("nope".into()))
    });
    assert!(matches!(got, Err(RunError::ProgramAbsent(_))));
    assert_eq!(n, 1, "an absent program will still be absent next time");
}

#[test]
fn wait_for_returns_when_the_condition_holds() {
    let start = Instant::now();
    let mut n = 0;
    let got = wait_for(
        "thing",
        Duration::from_secs(5),
        Duration::from_millis(5),
        || {
            n += 1;
            n >= 3
        },
    );
    assert!(got.is_ok());
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn wait_for_times_out_rather_than_reporting_success() {
    let got = wait_for(
        "thing",
        Duration::from_millis(80),
        Duration::from_millis(5),
        || false,
    );
    assert!(matches!(got, Err(RunError::Timeout { .. })));
}
