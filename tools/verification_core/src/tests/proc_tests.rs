use super::*;

#[test]
fn expect_code_wants_exactly_that_code() {
    // A self-test that feeds a broken input passes only on its failure code.
    let v = Run::new("sh")
        .arg("-c")
        .arg("exit 1")
        .expect_code("selftest must fail", 1);
    assert!(matches!(v, Verdict::Held));
    let v = Run::new("sh")
        .arg("-c")
        .arg("exit 0")
        .expect_code("selftest must fail", 1);
    assert!(
        matches!(v, Verdict::Failed(_)),
        "a hollow selftest must not read as a pass"
    );
}

#[test]
fn expect_ok_maps_an_absent_program_to_did_not_run() {
    let v = Run::new("tbd-not-real-8c21").expect_ok("thing must work");
    assert!(matches!(v, Verdict::DidNotRun(NotRun::ToolAbsent(_), _)));
    assert_eq!(v.into_exit(), 2);
}

#[test]
fn a_signal_and_a_timeout_stay_did_not_run() {
    let v = Run::new("sh")
        .arg("-c")
        .arg("kill -9 $$")
        .expect_ok("must finish");
    assert!(matches!(
        v,
        Verdict::DidNotRun(NotRun::Signalled { signal: 9, .. }, _)
    ));
    let v = Run::new("sleep")
        .arg("30")
        .timeout(std::time::Duration::from_millis(100))
        .expect_ok("must finish");
    assert!(matches!(v, Verdict::DidNotRun(NotRun::Timeout { .. }, _)));
}

#[test]
fn which_finds_and_misses() {
    assert!(which("sh").is_ok());
    assert!(matches!(
        which("tbd-not-real-4b77"),
        Err(NotRun::ToolAbsent(_))
    ));
}
