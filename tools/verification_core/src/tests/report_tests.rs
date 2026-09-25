use super::*;
use crate::verdict::{Kind, NotRun};

#[test]
fn all_held_is_clean_and_zero() {
    let mut r = Report::new("t");
    r.check(Verdict::Held).check(Verdict::Held);
    assert!(r.clean());
    assert_eq!(r.counts(), (2, 0, 0));
    assert_eq!(r.finish(), 0);
}

#[test]
fn violations_exit_one() {
    let mut r = Report::new("t");
    r.check(Verdict::Held).check(Verdict::failed("bad"));
    assert!(!r.clean());
    assert_eq!(r.finish(), 1);
}

#[test]
fn a_did_not_run_outranks_violations() {
    // The whole point: 0 violations over inputs nobody read is not a pass, and a mixed run
    // must not be reported with the milder of the two codes.
    let mut r = Report::new("t");
    r.check(Verdict::failed("bad")).check(Verdict::did_not_run(
        "x",
        Kind::Ban,
        NotRun::ToolAbsent("grep".into()),
    ));
    assert_eq!(r.counts(), (2, 1, 1));
    assert_eq!(r.finish(), 2);
}

#[test]
fn did_not_run_alone_still_exits_two() {
    let mut r = Report::new("t");
    r.check(Verdict::did_not_run(
        "x",
        Kind::Pin,
        NotRun::ToolAbsent("git".into()),
    ));
    assert_eq!(r.finish(), 2);
}

#[test]
fn an_empty_report_is_clean() {
    // Deliberate: "no checks defined" is a wiring bug, but it is not this type's job to
    // detect it. The gate runner lists every gate it runs.
    let r = Report::new("t");
    assert!(r.clean());
}
