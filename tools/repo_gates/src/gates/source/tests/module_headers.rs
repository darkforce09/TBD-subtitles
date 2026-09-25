use super::*;

const HEADER: &str = "//! Title.\n//!\n//! **Role:** r.\n//! **Position:** p.\n\
                      //! **Signals and state:** s.\n//! **Invariants:** i.\n\nfn f() {}\n";

#[test]
fn a_crate_root_needs_every_label_in_order() {
    assert!(
        ModuleHeaders
            .problems("crates/x/src/lib.rs", HEADER.as_bytes())
            .is_empty()
    );
    let missing = "//! Title.\n//! **Role:** r.\n//! **Invariants:** i.\n";
    assert_eq!(
        ModuleHeaders.problems("crates/x/src/lib.rs", missing.as_bytes()),
        [
            "crates/x/src/lib.rs: a crate root, but the leading //! header does not name \
             **Position:** after the labels before it",
            "crates/x/src/lib.rs: a crate root, but the leading //! header does not name \
             **Signals and state:** after the labels before it",
        ]
    );
}

#[test]
fn a_label_after_the_leading_block_does_not_count() {
    let late = "//! Title.\n\n//! **Role:** **Position:** **Signals and state:** **Invariants:**\n";
    assert_eq!(
        ModuleHeaders
            .problems("apps/a/src/main.rs", late.as_bytes())
            .len(),
        4
    );
}

#[test]
fn short_files_and_tests_need_no_header() {
    let short = "//! A small module.\nfn f() {}\n";
    assert!(
        ModuleHeaders
            .problems("crates/x/src/small.rs", short.as_bytes())
            .is_empty()
    );
    let long = format!("//! A long module.\n{}", "fn f() {}\n".repeat(LONG_FILE));
    assert_eq!(
        ModuleHeaders
            .problems("crates/x/src/long.rs", long.as_bytes())
            .len(),
        4
    );
    assert!(!ModuleHeaders.selects("crates/x/src/tests/long.rs"));
}
