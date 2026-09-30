use std::os::unix::process::ExitStatusExt;

use super::*;

#[test]
fn only_sigkill_counts_as_killed() {
    assert!(killed_by_sigkill(ExitStatus::from_raw(SIGKILL)));
    assert!(!killed_by_sigkill(ExitStatus::from_raw(15)));
    assert!(!killed_by_sigkill(ExitStatus::from_raw(1 << 8)));
}

#[test]
fn an_exit_names_its_code_or_its_signal() {
    assert_eq!(describe_exit(ExitStatus::from_raw(1 << 8)), "exited 1");
    assert_eq!(
        describe_exit(ExitStatus::from_raw(SIGKILL)),
        "ended by signal 9"
    );
}

#[test]
fn a_failure_line_follows_the_exit() {
    assert_eq!(
        with_failure(
            "exited 1".to_string(),
            Some("failed: write | Io".to_string())
        ),
        "exited 1 (failed: write | Io)"
    );
    assert_eq!(with_failure("exited 0".to_string(), None), "exited 0");
}
