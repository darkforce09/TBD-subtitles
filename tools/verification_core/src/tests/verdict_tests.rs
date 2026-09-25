use super::*;

#[test]
fn renders_a_bare_failure_as_one_headline() {
    let v = Verdict::failed("agent must never eval a request");
    assert_eq!(v.to_string(), "FAIL: agent must never eval a request");
}

#[test]
fn a_missing_target_names_the_file_and_the_six_space_continuation() {
    let v = Verdict::did_not_run(
        "socket must be 0600",
        Kind::Pin,
        NotRun::TargetMissing(PathBuf::from("etc/socket.conf")),
    );
    assert_eq!(
        v.to_string(),
        "FAIL: socket must be 0600 — target file missing: etc/socket.conf\n      \
         The pin could not run. A moved or deleted file must not read as a clean result."
    );
}

#[test]
fn ban_and_pin_differ_only_in_the_noun() {
    // Only TargetMissing uses the "The {noun} could not run." phrasing; every tool-failure
    // cause ends with "on a {noun} that did not execute".
    let ban = Verdict::did_not_run("m", Kind::Ban, NotRun::ToolAbsent("grep".into()));
    let pin = Verdict::did_not_run("m", Kind::Pin, NotRun::ToolAbsent("grep".into()));
    assert!(
        ban.to_string().ends_with("on a ban that did not execute."),
        "{ban}"
    );
    assert!(
        pin.to_string().ends_with("on a pin that did not execute."),
        "{pin}"
    );

    let ban_missing =
        Verdict::did_not_run("m", Kind::Ban, NotRun::TargetMissing(PathBuf::from("x")));
    let pin_missing =
        Verdict::did_not_run("m", Kind::Pin, NotRun::TargetMissing(PathBuf::from("x")));
    assert!(ban_missing.to_string().contains("The ban could not run"));
    assert!(pin_missing.to_string().contains("The pin could not run"));
}

#[test]
fn exit_codes_separate_did_not_run_from_failed() {
    assert_eq!(Verdict::Held.into_exit(), 0);
    assert_eq!(Verdict::failed("x").into_exit(), 1);
    let dnr = Verdict::did_not_run("x", Kind::Ban, NotRun::ToolAbsent("grep".into()));
    assert_eq!(
        dnr.into_exit(),
        2,
        "did-not-run must be distinguishable from a violation"
    );
}

#[test]
fn the_binary_exit_code_collapses_both_failure_kinds_to_one() {
    // What a caller on a two-outcome exit contract sees: 1 for a violation and 1 for a check
    // that never ran.
    assert_eq!(Verdict::Held.into_binary_exit_code(), 0);
    assert_eq!(Verdict::failed("x").into_binary_exit_code(), 1);
    let dnr = Verdict::did_not_run("x", Kind::Pin, NotRun::ToolAbsent("grep".into()));
    assert_eq!(dnr.into_binary_exit_code(), 1);
}

#[test]
fn signal_death_is_did_not_run_never_failed() {
    // THE CONTRACT. Synthesising 128+n from the signal yields a number the next `match` arm
    // reads as an ordinary failure; the OOM killer makes this routine on a full GPU job.
    let v = Verdict::did_not_run(
        "gpu worker",
        Kind::Pin,
        NotRun::Signalled {
            tool: "tbd-subtitles".into(),
            signal: 9,
        },
    );
    assert!(matches!(
        v,
        Verdict::DidNotRun(NotRun::Signalled { signal: 9, .. }, _)
    ));
    assert_eq!(v.into_exit(), 2);
}

#[test]
fn held_renders_nothing() {
    assert_eq!(Verdict::Held.to_string(), "");
}
