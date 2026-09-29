//! Module layout, dependency direction and file-size rules for the app's source tree.

#[path = "source_inspection.rs"]
mod source_inspection;

use source_inspection::{dependencies, is_test, resolve_path, rust_sources, tokens};
use std::path::{Path, PathBuf};

/// Every top-level module folder under `src/`.
const MODULES: &[&str] = &[
    "text_review",
    "application",
    "cli",
    "core",
    "job_queue",
    "job_report",
    "line_review",
    "log_console",
    "settings",
];

/// The modules that compose the features; every other module except `core` is a feature.
const COMPOSITION: &[&str] = &["application", "cli"];

fn source_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn is_feature(module: &str) -> bool {
    MODULES.contains(&module) && module != "core" && !COMPOSITION.contains(&module)
}

#[test]
fn source_files_respect_the_size_limits_without_exemptions() {
    let sources = rust_sources(&source_root());
    assert!(
        !sources.is_empty(),
        "the source scan must not pass vacuously"
    );
    let mut failures = Vec::new();
    for (path, text) in sources {
        let maximum = if is_test(&path) { 999 } else { 499 };
        let actual = text.lines().count();
        if actual > maximum {
            failures.push(format!(
                "{}: {actual} lines, maximum {maximum}",
                path.display()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn module_roots_and_documentation_describe_the_entire_source_tree() {
    let root = source_root();
    assert!(root.parent().unwrap().join("README.md").is_file());
    for entry in std::fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(
            if path.is_dir() {
                MODULES.contains(&name) || name == "tests"
            } else {
                matches!(name, "main.rs" | "README.md")
            },
            "unexpected flat source or module: {}",
            path.display()
        );
    }
    for name in MODULES {
        let module = root.join(name);
        assert!(
            module.join("mod.rs").is_file(),
            "missing module root: {name}"
        );
    }
    for feature in MODULES.iter().filter(|module| is_feature(module)) {
        for part in ["models", "services", "ui"] {
            assert!(
                root.join(feature).join(part).join("mod.rs").is_file(),
                "feature {feature} lacks {part}/mod.rs"
            );
        }
    }
}

#[test]
fn dependency_boundaries_and_external_test_placement_are_enforced() {
    let root = source_root();
    let mut failures = Vec::new();
    for (path, text) in rust_sources(&root) {
        if is_test(&path) {
            continue;
        }
        let relative = path.strip_prefix(&root).unwrap();
        let module = relative
            .components()
            .next()
            .unwrap()
            .as_os_str()
            .to_str()
            .unwrap();
        let code = tokens(&text);
        if code
            .windows(3)
            .any(|window| window[0] == "mod" && window[2] == "{")
            || code
                .windows(3)
                .any(|window| window[0] == "[" && window[1] == "test" && window[2] == "]")
        {
            failures.push(format!(
                "{}: modules and unit tests live in separate files",
                relative.display()
            ));
        }
        let pure = relative
            .components()
            .any(|part| matches!(part.as_os_str().to_str(), Some("models" | "services")));
        if pure
            && code
                .iter()
                .any(|token| ["egui", "eframe"].contains(&token.as_str()))
        {
            failures.push(format!(
                "{}: model/service code depends on rendering",
                relative.display()
            ));
        }
        for dependency in dependencies(&code) {
            let dependency = resolve_path(relative, &dependency);
            let Some(target) = dependency.first().map(String::as_str) else {
                continue;
            };
            let violation = if module == "core" && MODULES.contains(&target) && target != "core" {
                Some("core depends on a feature or the composition layer")
            } else if is_feature(module) && COMPOSITION.contains(&target) {
                Some("feature depends on the composition layer")
            } else if module != "application"
                && module != target
                && is_feature(target)
                && dependency.iter().any(|part| part == "ui")
            {
                Some("module imports another feature's UI")
            } else {
                None
            };
            if let Some(reason) = violation {
                failures.push(format!(
                    "{}: {reason}: {}",
                    relative.display(),
                    dependency.join("::")
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn source_inspection_handles_grouped_imports_aliases_and_ignored_prose() {
    let source = r###"
        // use crate::application::State;
        /* outer /* use crate::application; */ comment */
        const NOTE: &str = r#"use crate::application;"#;
        use crate::{job_queue::{models::View, ui as forbidden_ui}, core::ui};
        use super::super::services::load;
        fn action() { crate::settings::models::State::Closed; }
    "###;
    let code = tokens(source);
    let paths = dependencies(&code);
    assert!(!paths.iter().flatten().any(|part| part == "application"));
    assert!(paths.contains(&vec!["crate".into(), "job_queue".into(), "ui".into()]));
    assert!(paths.contains(&vec!["crate".into(), "core".into(), "ui".into()]));
    assert_eq!(
        resolve_path(
            Path::new("job_queue/ui/queue_panel.rs"),
            &[
                "super".into(),
                "super".into(),
                "services".into(),
                "load".into()
            ]
        ),
        ["job_queue", "services", "load"]
    );
}
