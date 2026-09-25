use super::*;

fn lines(count: usize) -> Vec<u8> {
    "x\n".repeat(count).into_bytes()
}

#[test]
fn production_files_stop_below_500_lines() {
    assert!(
        FileLength
            .problems("crates/x/src/a.rs", &lines(499))
            .is_empty()
    );
    assert_eq!(
        FileLength.problems("crates/x/src/a.rs", &lines(500)),
        [
            "crates/x/src/a.rs: 500 lines; a production file holds fewer than 500, so split it by \
          responsibility"
        ]
    );
}

#[test]
fn test_files_stop_below_1000_lines() {
    assert!(
        FileLength
            .problems("crates/x/src/tests/a.rs", &lines(999))
            .is_empty()
    );
    assert_eq!(
        FileLength
            .problems("crates/x/src/tests/a.rs", &lines(1000))
            .len(),
        1
    );
}

#[test]
fn only_rust_files_are_judged() {
    assert!(FileLength.selects("a/b.rs"));
    assert!(!FileLength.selects("a/b.md"));
}
