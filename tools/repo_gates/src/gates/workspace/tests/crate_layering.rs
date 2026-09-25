use super::*;
use crate::gate_run::GateRequest;
use crate::gate_run::file_rule::judge_file_rule;
use crate::gate_run::fixture_checkout::{failures, outcome_counts};
use crate::gate_run::tracked_tree::TrackedTree;

fn problems(manifest: &str) -> Vec<String> {
    CrateLayering.problems("crates/x/Cargo.toml", manifest.as_bytes())
}

#[test]
fn a_crate_may_depend_on_lower_layers_only() {
    let stages = "[package]\nname = \"stages\"\n[dependencies]\njob_model = { path = \"x\" }\n\
                  media_io = { path = \"x\" }\nserde = \"1\"\n";
    assert!(problems(stages).is_empty());
    let upward = "[package]\nname = \"media_io\"\n[dependencies]\nstages = { path = \"x\" }\n\
                  [dev-dependencies]\ninference = { path = \"x\" }\n";
    assert_eq!(
        problems(upward),
        [
            "crates/x/Cargo.toml: `media_io` may not depend on `stages`, which is not below it",
            "crates/x/Cargo.toml: `media_io` may not depend on `inference`, which is not below it",
        ],
        "an upward edge and a sibling edge both fail"
    );
}

#[test]
fn renames_and_target_tables_are_followed() {
    let manifest = "[package]\nname = \"job_model\"\n\
                    [target.'cfg(unix)'.dependencies]\nrunner = { package = \"pipeline\", path = \"x\" }\n";
    assert_eq!(problems(manifest).len(), 1);
}

#[test]
fn tools_use_only_their_listed_crates_and_unknown_crates_fail() {
    let gates = "[package]\nname = \"repo_gates\"\n[dependencies]\nverification_core = { path = \"x\" }\n\
                 stages = { path = \"x\" }\n";
    assert_eq!(problems(gates).len(), 1);
    let unknown = "[package]\nname = \"mystery\"\n";
    assert!(problems(unknown)[0].contains("is in no layer"));
    assert!(problems("not toml [")[0].contains("does not parse"));
}

#[test]
fn the_workspace_holds_its_layers() {
    let root = crate::test_repo_root();
    let run = judge_file_rule(
        &CrateLayering,
        &root,
        TrackedTree::load(&root, crate::gate_run::UntrackedFiles::Included),
        &GateRequest::default(),
    );
    assert_eq!(failures(&run), Vec::<String>::new());
    let (held, _, _) = outcome_counts(&run);
    assert!(
        held >= PRODUCT_LAYERS.len(),
        "every member manifest is judged"
    );
}
