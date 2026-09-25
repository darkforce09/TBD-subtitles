use super::*;
use crate::cli::{Cli, Gate};
use clap::{Parser, ValueEnum};

fn tree() -> TrackedTree {
    TrackedTree::from_listing("apps/a/b/c.rs\0apps/a/README.md\0apps/bc/d.rs\0tools/x.rs")
}

fn resolve(values: &[&str]) -> Result<GateScope, ScopeRefusal> {
    let values: Vec<String> = values.iter().map(ToString::to_string).collect();
    GateScope::resolve(&values, Path::new("/checkout"), &tree())
}

#[test]
fn no_value_and_the_root_spellings_mean_the_whole_repository() {
    let cases: [&[&str]; 5] = [&[], &["."], &["./"], &["/checkout"], &["apps/a", "."]];
    for values in cases {
        let scope = resolve(values).expect("a whole-repository scope");
        assert_eq!(scope.describe(), "the whole repository", "{values:?}");
        assert!(scope.contains("tools/x.rs"));
        assert_eq!(scope.anchor(Path::new("/checkout")), Path::new("/checkout"));
    }
}

#[test]
fn a_folder_value_is_normalised_and_selects_its_subtree_only() {
    let scope = resolve(&["./apps//a/"]).expect("a tracked folder");
    assert_eq!(scope.describe(), "apps/a");
    assert!(scope.contains("apps/a"));
    assert!(scope.contains("apps/a/b/c.rs"));
    assert!(!scope.contains("apps"));
    assert!(!scope.contains("apps/ab/x.rs"));
    assert_eq!(
        scope.anchor(Path::new("/checkout")),
        Path::new("/checkout/apps/a")
    );
}

#[test]
fn an_absolute_value_inside_the_checkout_is_relative_to_it() {
    let scope = resolve(&["/checkout/apps/bc"]).expect("inside the checkout");
    assert_eq!(scope.describe(), "apps/bc");
}

#[test]
fn repeated_values_add_folders_once_each() {
    let scope = resolve(&["apps/a", "tools", "apps/a/"]).expect("tracked folders");
    assert_eq!(scope.describe(), "apps/a, tools");
    assert!(scope.contains("tools/x.rs"));
    assert!(!scope.contains("apps/bc/d.rs"));
}

#[test]
fn a_value_that_names_no_tracked_folder_is_refused() {
    let cases = [
        ("apps/missing", "names no tracked folder"),
        ("apps/a/b/c.rs", "is a file; --path takes a folder"),
        ("apps/../tools", "climbs out of its folder with `..`"),
        ("/elsewhere/apps", "lies outside the checkout"),
    ];
    for (value, reason) in cases {
        let refusal = resolve(&[value]).expect_err(value);
        assert_eq!(
            refusal,
            ScopeRefusal {
                value: value.to_string(),
                reason
            }
        );
    }
}

#[test]
fn a_bad_value_is_refused_even_beside_a_whole_repository_value() {
    let refusal = resolve(&[".", "apps/missing"]).expect_err("the typo must be refused");
    assert_eq!(refusal.value, "apps/missing");
}

#[test]
fn every_gate_takes_a_repeatable_path_and_the_untracked_flag() {
    for gate in Gate::all() {
        let verb = gate
            .to_possible_value()
            .expect("every gate is named")
            .get_name()
            .to_string();
        let cli = Cli::try_parse_from([
            "cargo gates",
            &verb,
            "--path",
            "apps",
            "--with-untracked",
            "--path",
            "tools",
        ])
        .expect("the gate parses");
        assert_eq!(cli.gate, Some(*gate));
        assert_eq!(cli.arguments.paths, ["apps", "tools"], "{verb}");
        assert!(cli.arguments.with_untracked, "{verb}");
        let bare = Cli::try_parse_from(["cargo gates", &verb]).expect("no flags");
        assert!(bare.arguments.paths.is_empty(), "{verb}");
        assert!(
            !bare.arguments.with_untracked,
            "{verb}: the committed view is the default"
        );
    }
    let all = Cli::try_parse_from(["cargo gates"]).expect("no gate named");
    assert_eq!(all.gate, None, "no gate named runs every gate");
}
