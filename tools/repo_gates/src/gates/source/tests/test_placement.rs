use super::*;

fn problems(text: &str) -> Vec<String> {
    TestPlacement.problems("crates/x/src/a.rs", text.as_bytes())
}

#[test]
fn a_sibling_test_file_passes() {
    let text = "fn f() {}\n\n#[cfg(test)]\n#[path = \"tests/a.rs\"]\nmod tests;\n";
    assert!(problems(text).is_empty());
}

#[test]
fn inline_modules_test_functions_and_unplaced_files_fail() {
    let text = "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n\n\
                #[cfg(any(test, feature = \"x\"))]\nmod helpers;\n\n\
                fn outer() {\n    #[cfg(test)]\n    mod nested {}\n}\n";
    assert_eq!(
        problems(text),
        [
            "crates/x/src/a.rs:2: test module `tests` is inline; move it to a file under tests/",
            "crates/x/src/a.rs:4: test function `t` sits in a production file",
            "crates/x/src/a.rs:8: test module `helpers` needs #[path = \"tests/<file>.rs\"]",
            "crates/x/src/a.rs:12: test module `nested` is inline; move it to a file under tests/",
        ]
    );
}

#[test]
fn strings_and_comments_never_count() {
    let text = "// #[cfg(test)] mod tests { }\nconst S: &str = \"#[test] fn t() {}\";\n";
    assert!(problems(text).is_empty());
}

#[test]
fn a_file_that_does_not_parse_fails() {
    let found = problems("fn (\n");
    assert_eq!(found.len(), 1);
    assert!(
        found[0].starts_with("crates/x/src/a.rs:1: does not parse"),
        "{found:?}"
    );
}
