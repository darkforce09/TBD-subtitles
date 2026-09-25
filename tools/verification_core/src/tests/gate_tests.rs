use super::*;
use std::io::Write;

/// A scratch file that cleans itself up. Avoids a dev-dependency for six tests.
struct Tmp(std::path::PathBuf);
impl Tmp {
    fn new(name: &str, body: &str) -> Tmp {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "verification-core-gate-{}-{}",
            std::process::id(),
            name
        ));
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        Tmp(p)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn pat(p: &str) -> Pattern {
    Pattern::regex(p).unwrap()
}

#[test]
fn ban_holds_when_absent() {
    let f = Tmp::new("ban-clean", "all good here\n");
    assert!(matches!(
        ban("no evil", &pat("evil"), &[f.path()]),
        Verdict::Held
    ));
}

#[test]
fn ban_fails_when_present() {
    let f = Tmp::new("ban-dirty", "some evil here\n");
    let v = ban("no evil", &pat("evil"), &[f.path()]);
    assert!(matches!(v, Verdict::Failed(_)));
    assert_eq!(v.to_string(), "FAIL: no evil");
}

#[test]
fn require_fails_when_absent() {
    let f = Tmp::new("req-missing", "nothing relevant\n");
    assert!(matches!(
        require("must pin", &pat("PINNED"), &[f.path()]),
        Verdict::Failed(_)
    ));
}

#[test]
fn require_holds_when_present() {
    let f = Tmp::new("req-ok", "PINNED = 1\n");
    assert!(matches!(
        require("must pin", &pat("PINNED"), &[f.path()]),
        Verdict::Held
    ));
}

/// THE DEFECT THIS CRATE EXISTS FOR. A missing target must never read as a clean ban.
#[test]
fn missing_target_is_did_not_run_not_held() {
    let v = ban(
        "no evil",
        &pat("evil"),
        &[Path::new("/nonexistent/verification-core/nope")],
    );
    assert!(matches!(v, Verdict::DidNotRun(NotRun::TargetMissing(_), _)));
    assert_ne!(v.into_exit(), 0, "a check that did not run must not exit 0");
}

#[test]
fn missing_target_on_require_is_also_did_not_run() {
    let v = require(
        "must pin",
        &pat("x"),
        &[Path::new("/nonexistent/verification-core/nope")],
    );
    assert!(matches!(v, Verdict::DidNotRun(NotRun::TargetMissing(_), _)));
}

#[test]
fn a_directory_target_is_missing_not_unreadable() {
    let v = ban("no evil", &pat("evil"), &[Path::new("/tmp")]);
    assert!(matches!(v, Verdict::DidNotRun(NotRun::TargetMissing(_), _)));
}

#[test]
fn one_missing_among_many_fails_the_whole_check() {
    let f = Tmp::new("multi", "clean\n");
    let v = ban(
        "no evil",
        &pat("evil"),
        &[f.path(), Path::new("/nonexistent/verification-core/x")],
    );
    assert!(matches!(v, Verdict::DidNotRun(NotRun::TargetMissing(_), _)));
}

#[test]
fn patterns_cannot_match_across_a_file_boundary() {
    // A match must name one file an operator can open; text spanning two never counts.
    let a = Tmp::new("join-a", "prefix");
    let b = Tmp::new("join-b", "suffix\n");
    let v = ban("no join", &pat("prefixsuffix"), &[a.path(), b.path()]);
    assert!(matches!(v, Verdict::Held));
}

#[test]
fn str_helpers_need_no_files() {
    assert!(matches!(ban_str("no x", &pat("x"), "clean"), Verdict::Held));
    assert!(matches!(
        ban_str("no x", &pat("x"), "has x"),
        Verdict::Failed(_)
    ));
    assert!(matches!(
        require_str("want x", &pat("x"), "has x"),
        Verdict::Held
    ));
    assert!(matches!(
        require_str("want x", &pat("x"), "clean"),
        Verdict::Failed(_)
    ));
}

#[test]
fn probe_propagates_did_not_run_instead_of_short_circuiting_clean() {
    // The compound-condition footgun: an unreadable input must not answer `false`.
    let got: Result<bool, NotRun> = probe_files(&pat("x"), &[Path::new("/nonexistent/tbd/x")]);
    assert!(matches!(got, Err(NotRun::TargetMissing(_))));
}

#[test]
fn probe_reports_both_true_and_false_when_it_ran() {
    let f = Tmp::new("probe", "alpha\n");
    assert!(probe_files(&pat("alpha"), &[f.path()]).unwrap());
    assert!(!probe_files(&pat("beta"), &[f.path()]).unwrap());
    assert!(probe_str(&pat("alpha"), "alpha").unwrap());
}
