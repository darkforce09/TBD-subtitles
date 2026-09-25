use super::*;

#[test]
fn caret_is_a_line_anchor() {
    // THE TRAP. Default regex-crate semantics would fail this, and every `^`-anchored ban would
    // go quietly green over a file full of violations.
    let p = Pattern::regex("^forbidden").unwrap();
    assert!(p.is_match("ok line\nforbidden line\n"));
}

#[test]
fn dollar_is_a_line_anchor() {
    let p = Pattern::regex("trailing$").unwrap();
    assert!(p.is_match("has trailing\nmore\n"));
}

#[test]
fn dot_does_not_cross_a_newline() {
    // Matching is per-line, so a `.` run can never span lines.
    let p = Pattern::regex("a.*b").unwrap();
    assert!(!p.is_match("a\nb"));
    assert!(p.is_match("a x b"));
}

#[test]
fn literal_escapes_metacharacters() {
    let p = Pattern::literal("foo(bar)");
    assert!(p.is_match("call foo(bar) here"));
    assert!(!p.is_match("call fooXbar here"));
}

#[test]
fn case_insensitive_folds() {
    let p = Pattern::regex("unsafe")
        .unwrap()
        .case_insensitive()
        .unwrap();
    assert!(p.is_match("UNSAFE block"));
    let sensitive = Pattern::regex("unsafe").unwrap();
    assert!(!sensitive.is_match("UNSAFE block"));
}

#[test]
fn posix_classes_work_as_in_an_extended_regex() {
    let p = Pattern::regex("foo[[:space:]]+bar").unwrap();
    assert!(p.is_match("foo   bar"));
}

#[test]
fn source_survives_escaping_for_diagnostics() {
    assert_eq!(Pattern::literal("a.b").source(), "a.b");
}

#[test]
fn invalid_regex_is_an_error_not_a_panic() {
    assert!(Pattern::regex("a(").is_err());
}
