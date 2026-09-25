//! Source inspection for architecture checks, without interpreting comments or string literals.

use std::path::{Path, PathBuf};

pub(super) fn rust_sources(directory: &Path) -> Vec<(PathBuf, String)> {
    let mut result = Vec::new();
    for entry in std::fs::read_dir(directory).expect("read source directory") {
        let path = entry.expect("read source entry").path();
        if path.is_dir() {
            result.extend(rust_sources(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let text = std::fs::read_to_string(&path).expect("read Rust source");
            result.push((path, text));
        }
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

pub(super) fn is_test(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str() == "tests")
}

/// Keep identifiers and path punctuation, masking nested comments and ordinary/raw strings.
pub(super) fn tokens(source: &str) -> Vec<String> {
    let bytes = source.as_bytes();
    let mut result = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at..].starts_with(b"//") {
            while at < bytes.len() && bytes[at] != b'\n' {
                at += 1;
            }
        } else if bytes[at..].starts_with(b"/*") {
            at += 2;
            let mut depth = 1;
            while at < bytes.len() && depth > 0 {
                if bytes[at..].starts_with(b"/*") {
                    depth += 1;
                    at += 2;
                } else if bytes[at..].starts_with(b"*/") {
                    depth -= 1;
                    at += 2;
                } else {
                    at += 1;
                }
            }
            assert_eq!(depth, 0, "unterminated block comment");
        } else if bytes[at] == b'r' && raw_string_end(bytes, at).is_some() {
            at = raw_string_end(bytes, at).expect("raw string end");
        } else if bytes[at] == b'"' {
            at += 1;
            while at < bytes.len() {
                match bytes[at] {
                    b'\\' => at += 2,
                    b'"' => {
                        at += 1;
                        break;
                    }
                    _ => at += 1,
                }
            }
        } else if bytes[at].is_ascii_alphabetic() || bytes[at] == b'_' {
            let start = at;
            at += 1;
            while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_') {
                at += 1;
            }
            result.push(source[start..at].to_owned());
        } else if bytes[at..].starts_with(b"::") {
            result.push("::".into());
            at += 2;
        } else {
            if b"{};,=*[]".contains(&bytes[at]) {
                result.push(char::from(bytes[at]).to_string());
            }
            at += 1;
        }
    }
    result
}

fn raw_string_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut quote = start + 1;
    while bytes.get(quote) == Some(&b'#') {
        quote += 1;
    }
    if bytes.get(quote) != Some(&b'"') {
        return None;
    }
    let hashes = quote - start - 1;
    let mut at = quote + 1;
    while at < bytes.len() {
        if bytes[at] == b'"'
            && bytes
                .get(at + 1..at + 1 + hashes)
                .is_some_and(|tail| tail.iter().all(|byte| *byte == b'#'))
        {
            return Some(at + 1 + hashes);
        }
        at += 1;
    }
    panic!("unterminated raw string");
}

/// Expand grouped `use` trees and collect fully qualified crate paths in expressions.
pub(super) fn dependencies(tokens: &[String]) -> Vec<Vec<String>> {
    let mut paths = Vec::new();
    let mut at = 0;
    while at < tokens.len() {
        if tokens[at] == "use" {
            at += 1;
            use_tree(tokens, &mut at, Vec::new(), &mut paths);
        } else if tokens[at] == "crate" && tokens.get(at + 1).is_some_and(|token| token == "::") {
            let mut path = vec!["crate".into()];
            at += 1;
            while tokens.get(at).is_some_and(|token| token == "::") {
                at += 1;
                if let Some(token) = tokens.get(at) {
                    if token == "{" {
                        break;
                    }
                    path.push(token.clone());
                    at += 1;
                }
            }
            paths.push(path);
        } else {
            at += 1;
        }
    }
    paths
}

fn use_tree(
    tokens: &[String],
    at: &mut usize,
    mut prefix: Vec<String>,
    paths: &mut Vec<Vec<String>>,
) {
    while let Some(token) = tokens.get(*at) {
        match token.as_str() {
            "{" => {
                *at += 1;
                while tokens.get(*at).is_some_and(|token| token != "}") {
                    use_tree(tokens, at, prefix.clone(), paths);
                    if tokens.get(*at).is_some_and(|token| token == ",") {
                        *at += 1;
                    }
                }
                assert!(
                    tokens.get(*at).is_some_and(|token| token == "}"),
                    "unterminated use tree"
                );
                *at += 1;
                return;
            }
            "," | "}" | ";" => {
                paths.push(prefix);
                return;
            }
            "as" => {
                *at += 2;
                paths.push(prefix);
                return;
            }
            "::" => {
                *at += 1;
            }
            _ => {
                prefix.push(token.clone());
                *at += 1;
            }
        }
    }
    paths.push(prefix);
}

pub(super) fn resolve_path(source: &Path, dependency: &[String]) -> Vec<String> {
    let mut module: Vec<String> = source
        .with_extension("")
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    if module.last().is_some_and(|part| part == "mod") {
        module.pop();
    }
    let mut path = dependency.to_vec();
    match path.first().map(String::as_str) {
        Some("crate") => {
            path.remove(0);
            path
        }
        Some("self" | "super") => {
            if path[0] == "self" {
                path.remove(0);
            }
            while path.first().is_some_and(|part| part == "super") {
                module.pop();
                path.remove(0);
            }
            module.extend(path);
            module
        }
        _ => path,
    }
}
