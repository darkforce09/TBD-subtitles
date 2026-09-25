use super::*;

fn problems(path: &str, bytes: &[u8]) -> Vec<String> {
    EditorConfig.problems(path, bytes)
}

#[test]
fn a_clean_file_and_an_empty_file_pass() {
    assert!(problems("a.rs", b"fn f() {}\n").is_empty());
    assert!(problems("a.rs", b"").is_empty());
}

#[test]
fn every_rule_fires_on_the_first_line_that_breaks_it() {
    assert_eq!(
        problems("a.rs", b"ok\r\nbad \nlast"),
        [
            "a.rs:1: carriage return; lines end with LF alone",
            "a.rs:2: trailing whitespace",
            "a.rs:3: no newline at the end of the file",
        ]
    );
    assert_eq!(problems("a.rs", b"ok\n\xff\n"), ["a.rs:2: not UTF-8"]);
}

#[test]
fn markdown_may_end_a_line_in_a_hard_break() {
    assert!(problems("a.md", b"line  \nnext\n").is_empty());
    assert_eq!(problems("a.md", b"line\r\n").len(), 1);
}
