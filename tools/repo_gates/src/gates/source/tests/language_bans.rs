use super::*;

fn problems(path: &str, text: &str) -> Vec<String> {
    LanguageBans.problems(path, text.as_bytes())
}

#[test]
fn rust_markdown_toml_json_and_the_configuration_files_pass() {
    for path in [
        "apps/tool/src/main.rs",
        "documentation/README.md",
        "Cargo.toml",
        "crates/x/schema.json",
        "Cargo.lock",
        ".gitignore",
        "crates/x/.gitattributes",
        ".editorconfig",
        "crates/x/src/tests/fixtures/sample.srt",
    ] {
        assert!(problems(path, "text\n").is_empty(), "{path}");
    }
}

#[test]
fn scripts_build_files_and_stray_kinds_are_banned() {
    for path in [
        "tools/build.sh",
        "tools/convert.py",
        "Makefile",
        "tools/GNUmakefile",
        "tools/package.json",
        "tools/run.mjs",
        "tools/index.ts",
        "tools/run.ps1",
        "notes.txt",
        "crates/x/sample.srt",
    ] {
        assert_eq!(problems(path, "text\n").len(), 1, "{path}");
    }
}

#[test]
fn a_shebang_naming_a_banned_interpreter_fails_even_in_an_allowed_file() {
    assert_eq!(
        problems("tools/run.rs", "#!/usr/bin/env -S python3 -u\nprint()\n"),
        ["tools/run.rs:1: shebang names `python3`"]
    );
    assert_eq!(
        problems("tools/run", "#!/bin/bash\n").len(),
        2,
        "no allowed kind, and a shell shebang"
    );
    assert!(problems("tools/lib.rs", "#![allow(dead_code)]\n").is_empty());
}

#[test]
fn the_shebang_is_parsed_not_matched() {
    assert_eq!(shebang_interpreter("#!/bin/sh"), Some("sh"));
    assert_eq!(shebang_interpreter("#!/usr/bin/env node"), Some("node"));
    assert_eq!(shebang_interpreter("#!/usr/bin/env"), None);
    assert_eq!(shebang_interpreter("# bash"), None);
    assert_eq!(
        shebang_interpreter("#![allow(clippy::all)]"),
        Some("[allow(clippy::all)]")
    );
}
