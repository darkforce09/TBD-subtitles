use std::sync::Mutex;

use super::*;

/// `HOME` is process-global; serialize tests that touch it.
static HOME_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn resolve_program_falls_back_to_home_local_bin() {
    let _guard = HOME_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("tbd-claude-cli-test-{}", std::process::id()));
    let bin = dir.join(".local/bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("claude"), b"").unwrap();
    let old_home = std::env::var_os("HOME");
    let old_path = std::env::var_os("PATH");
    // SAFETY: guarded by `HOME_LOCK`, restored before the guard drops.
    unsafe {
        std::env::set_var("HOME", &dir);
        std::env::set_var("PATH", "");
    }

    let resolved = resolve_program();

    // SAFETY: guarded by `HOME_LOCK`.
    unsafe {
        match old_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match old_path {
            Some(v) => std::env::set_var("PATH", v),
            None => std::env::remove_var("PATH"),
        }
    }
    std::fs::remove_dir_all(&dir).ok();

    assert_eq!(resolved, bin.join("claude").to_string_lossy());
}

#[test]
fn resolve_program_falls_back_to_bare_name_when_nowhere_found() {
    let _guard = HOME_LOCK.lock().unwrap();
    let old_home = std::env::var_os("HOME");
    let old_path = std::env::var_os("PATH");
    // SAFETY: guarded by `HOME_LOCK`, restored before the guard drops.
    unsafe {
        std::env::set_var("HOME", "/nonexistent-tbd-test-home");
        std::env::set_var("PATH", "");
    }

    let resolved = resolve_program();

    // SAFETY: guarded by `HOME_LOCK`.
    unsafe {
        match old_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match old_path {
            Some(v) => std::env::set_var("PATH", v),
            None => std::env::remove_var("PATH"),
        }
    }

    assert_eq!(resolved, "claude");
}

#[test]
fn reads_the_structured_output_tokens_and_cost() {
    let stdout = r#"{"is_error":false,"total_cost_usd":0.0054,
      "usage":{"input_tokens":2,"cache_creation_input_tokens":1187,"cache_read_input_tokens":10,"output_tokens":64},
      "structured_output":{"t":"hello"}}"#;
    let c = parse(stdout).unwrap();
    assert_eq!(c.json["t"], "hello");
    assert_eq!((c.input_tokens, c.output_tokens), (1199, 64));
    assert_eq!(c.cost_usd, Some(0.0054));
}

#[test]
fn a_result_without_structured_output_is_an_error() {
    assert!(parse(r#"{"is_error":false,"result":"text"}"#).is_err());
    assert!(parse(r#"{"is_error":true,"result":"limit reached"}"#).is_err());
    assert!(parse("not json").is_err());
}

#[test]
fn a_cancellable_run_hands_back_the_whole_stdout() {
    // The `PATH` tests above clear it for a moment.
    let _guard = HOME_LOCK.lock().unwrap();
    let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let run = Run::new("echo").arg("hello").cancel_on(flag);
    let (code, stdout, _) = run_cancellable(run).unwrap();
    assert_eq!((code, stdout.as_str()), (0, "hello\n"));
}

#[test]
fn a_set_cancel_flag_stops_the_call() {
    let _guard = HOME_LOCK.lock().unwrap();
    let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let setter = flag.clone();
    let started = std::time::Instant::now();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        setter.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    let run = Run::new("sleep").arg("20").cancel_on(flag);
    let error = run_cancellable(run).unwrap_err();
    assert_eq!(error.0, "cancelled");
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_backend_with_a_cancel_flag_keeps_it() {
    let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let cli = ClaudeCli::new("opus", std::env::temp_dir()).with_cancel(flag);
    assert!(cli.cancel.is_some());
    assert_eq!(cli.name(), "claude-cli/opus");
}
