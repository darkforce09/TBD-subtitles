use std::path::Path;

use clap::{Arg, ArgAction, ValueEnum, value_parser};

use super::super::markdown_scan::scan;
use super::super::{Break, BreakListing, judge as judge_gate};
use super::*;
use crate::gate_run::GateRequest;
use crate::gate_run::fixture_checkout::{FixtureCheckout, failures, outcome_counts};
use crate::gate_run::tracked_tree::TrackedTree;
use crate::layout::FROZEN_FOLDERS;

/// The values of the fixture's `run` argument, one of them with an alias.
#[derive(Clone, ValueEnum)]
enum Pace {
    #[value(alias = "quick")]
    Fast,
    Thorough,
}

/// A small command tree shaped like xtask's: groups with verbs, an alias, a group option that
/// takes a value, a leaf with arguments, a leaf group root, a leaf whose first argument is a value
/// enum behind an option and a switch, and a group whose own argument declares a list of values.
fn fixture_tree() -> Command {
    let mut tree = Command::new("xtask")
        .disable_help_subcommand(true)
        .subcommand(
            Command::new("ticket")
                .subcommand(
                    Command::new("check")
                        .arg(Arg::new("strict").long("strict").action(ArgAction::SetTrue)),
                )
                .subcommand(Command::new("show").arg(Arg::new("id")))
                .subcommand(Command::new("list").alias("ls")),
        )
        .subcommand(
            Command::new("verify")
                .arg(Arg::new("format").long("format").short('f'))
                .arg(
                    Arg::new("quiet")
                        .long("quiet")
                        .short('q')
                        .action(ArgAction::SetTrue),
                )
                .subcommand(Command::new("link-check")),
        )
        .subcommand(Command::new("ci").arg(Arg::new("target")))
        .subcommand(
            Command::new("run")
                .arg(Arg::new("format").long("format"))
                .arg(Arg::new("verbose").short('v').action(ArgAction::SetTrue))
                .arg(Arg::new("pace").value_parser(value_parser!(Pace)))
                .arg(Arg::new("rest").num_args(0..)),
        )
        .subcommand(
            Command::new("switch")
                .arg(Arg::new("state").value_parser(["on", "off"]))
                .subcommand(Command::new("status")),
        );
    tree.build();
    tree
}

fn walk(tree: &Command, text: &str) -> CitedCommand {
    let rest = citations(text)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("`{text}` cites no command"));
    walk_command(tree, &command_words(rest))
}

fn unknown(path: &str) -> CitedCommand {
    CitedCommand::Unknown(path.to_string())
}

/// What the rule makes of one document judged alone over the fixture tree: every break as it
/// renders, and the totals line. The rule reads neither the checkout nor its tracked tree.
fn judge_document(text: &str) -> (Vec<String>, String) {
    let tree = fixture_tree();
    let tracked = TrackedTree::default();
    let context = RuleContext {
        repo_root: Path::new("/nonexistent/command-citations"),
        tree: &tracked,
    };
    let scanned = scan(text);
    let mut rule = CommandCitations::new(&tree);
    let mut findings = RuleFindings::default();
    rule.judge(
        &JudgedDocument {
            path: "apps/tool/README.md",
            scan: &scanned,
        },
        &context,
        &mut findings,
    );
    let breaks = findings.breaks.iter().map(Break::render).collect();
    (breaks, rule.totals().join("\n"))
}

#[test]
fn a_group_and_its_verb_exist() {
    let tree = fixture_tree();
    assert_eq!(
        walk(&tree, "cargo gates ticket check"),
        CitedCommand::Exists
    );
    assert_eq!(
        walk(&tree, "cargo gates verify link-check"),
        CitedCommand::Exists
    );
    assert_eq!(walk(&tree, "cargo gates ticket"), CitedCommand::Exists);
    assert_eq!(walk(&tree, "cargo gates"), CitedCommand::Exists);
    assert_eq!(walk(&tree, "cargo gates --help"), CitedCommand::Exists);
}

#[test]
fn an_unknown_group_or_verb_names_the_path_so_far() {
    let tree = fixture_tree();
    assert_eq!(
        walk(&tree, "cargo gates tickets check"),
        unknown("cargo gates tickets")
    );
    assert_eq!(
        walk(&tree, "cargo gates ticket milestone --all"),
        unknown("cargo gates ticket milestone")
    );
    assert_eq!(
        walk(&tree, "cargo gates ticket help"),
        unknown("cargo gates ticket help"),
        "a group whose tree has no help subcommand refuses one"
    );
}

#[test]
fn the_words_after_a_leaf_are_arguments() {
    let tree = fixture_tree();
    assert_eq!(
        walk(&tree, "cargo gates ticket show T-042 extra words"),
        CitedCommand::Exists
    );
    assert_eq!(walk(&tree, "cargo gates ci ci-local"), CitedCommand::Exists);
    assert_eq!(
        walk(&tree, "cargo gates verify link-check anything at all"),
        CitedCommand::Exists
    );
}

#[test]
fn flags_are_skipped_and_a_flag_that_takes_a_value_takes_it_along() {
    let tree = fixture_tree();
    for text in [
        "cargo gates --verbose ticket check",
        "cargo gates verify --format json link-check",
        "cargo gates verify -f json link-check",
        "cargo gates verify --format=json link-check",
        "cargo gates verify -q link-check",
        "cargo gates verify --quiet link-check",
    ] {
        assert_eq!(walk(&tree, text), CitedCommand::Exists, "{text}");
    }
    assert_eq!(
        walk(&tree, "cargo gates verify --quiet json link-check"),
        unknown("cargo gates verify json"),
        "a switch takes no value"
    );
}

#[test]
fn a_placeholder_where_a_subcommand_belongs_ends_the_walk() {
    let tree = fixture_tree();
    for text in [
        "cargo gates ticket <verb>",
        "cargo gates ticket [verb] T-042",
        "cargo gates ticket {check,show}",
        "cargo gates ticket …",
        "cargo gates ...",
        "cargo gates <group> <verb>",
    ] {
        assert_eq!(
            walk(&tree, text),
            CitedCommand::ReachesPlaceholder,
            "{text}"
        );
    }
}

#[test]
fn an_alias_names_its_subcommand() {
    let tree = fixture_tree();
    assert_eq!(walk(&tree, "cargo gates ticket ls"), CitedCommand::Exists);
}

#[test]
fn shell_syntax_ends_the_command() {
    let tree = fixture_tree();
    for text in [
        "cargo gates ticket | tee out.log",
        "cargo gates ticket|tee out.log",
        "cargo gates ticket && echo done",
        "cargo gates ticket; echo done",
        "cargo gates ticket > out.log",
        "cargo gates ticket 2>&1",
        "cargo gates ticket # every verb",
        "echo $(cargo gates ticket)",
        "echo \"run cargo gates ticket\" first",
        "cargo gates ticket &",
    ] {
        assert_eq!(walk(&tree, text), CitedCommand::Exists, "{text}");
    }
    assert_eq!(
        walk(&tree, "cargo gates ticket frobnicate;"),
        unknown("cargo gates ticket frobnicate")
    );
    for text in [
        "(cargo gates ticket check, wave repack, ticketboard verify)",
        "Run cargo gates ticket check.",
        "cargo gates ticket: every verb",
    ] {
        assert_eq!(
            walk(&tree, text),
            CitedCommand::Exists,
            "{text}: prose punctuation ends the command"
        );
    }
    assert_eq!(
        walk(&tree, "cargo gates ticket frobnicate, then check"),
        unknown("cargo gates ticket frobnicate")
    );
    assert_eq!(
        walk(&tree, "cargo gates \"ticket\" 'check'"),
        CitedCommand::Exists,
        "quotes around a word are the shell's, not the word's"
    );
}

#[test]
fn every_citation_in_a_text_is_found_and_hcargo_is_none() {
    assert_eq!(
        citations("cargo gates db up && cargo  gates db seed"),
        [" db up && cargo  gates db seed", " db seed"]
    );
    for text in [
        "hcargo gates ticket frobnicate",
        "$HOME/.cache/tbd-bin/hcargo gates ticket frobnicate",
        "cargo-gates ticket",
        "cargo gatess ticket",
        "cargo run -p gates -- ticket",
        "my_cargo gates ticket",
    ] {
        assert!(citations(text).is_empty(), "{text}");
    }
    assert_eq!(citations("(cargo gates)"), [")"]);
}

#[test]
fn a_fenced_line_ending_in_a_backslash_continues_on_the_next() {
    let block = CodeBlock {
        line: 10,
        info: "bash".to_string(),
        lines: vec![
            "cargo gates \\".to_string(),
            "  ticket \\  ".to_string(),
            "  check".to_string(),
            "echo next".to_string(),
            "cargo gates ticket \\".to_string(),
        ],
    };
    assert_eq!(
        shell_lines(&block),
        [
            (11, "cargo gates    ticket    check".to_string()),
            (14, "echo next".to_string()),
            (15, "cargo gates ticket  ".to_string()),
        ]
    );
}

#[test]
fn inline_spans_and_fenced_lines_are_judged_at_their_lines() {
    let text = "# Tool\n\nRun `cargo gates ticket check` or `cargo gates tickets`.\n\n\
                ```bash\n# first\ncargo gates ticket check --strict\ncargo gates ticket \\\n  \
                frobnicate\n```\n\n~~~text\ncargo gates verify link-check && cargo gates nope\n~~~\n\n\
                Plain prose cargo gates nope is no code.\n";
    let (breaks, totals) = judge_document(text);
    assert_eq!(
        breaks,
        [
            "apps/tool/README.md:3: cited command does not exist: `cargo gates tickets`",
            "apps/tool/README.md:8: cited command does not exist: `cargo gates ticket \
             frobnicate`",
            "apps/tool/README.md:13: cited command does not exist: `cargo gates nope`",
        ]
    );
    assert!(
        totals.contains(
            "6 judged in live documents — 3 name an existing command, 0 reach a placeholder, 3 \
             name no command"
        ),
        "{totals}"
    );
}

#[test]
fn the_gates_tree_resolves_its_own_commands() {
    let tree = gates_command_tree();
    for text in [
        "cargo gates",
        "cargo gates link-check --report",
        "cargo gates readme-coverage --path apps --with-untracked",
        "cargo gates crate-layering",
        "cargo gates --help",
    ] {
        assert_eq!(walk(&tree, text), CitedCommand::Exists, "{text}");
    }
    assert_eq!(
        walk(&tree, "cargo gates no-such-gate"),
        unknown("cargo gates no-such-gate")
    );
}

#[test]
fn a_value_enum_argument_takes_one_of_its_values_or_an_alias() {
    let tree = fixture_tree();
    for text in [
        "cargo gates run fast",
        "cargo gates run thorough",
        "cargo gates run quick",
        "cargo gates run fast then any words",
    ] {
        assert_eq!(walk(&tree, text), CitedCommand::Exists, "{text}");
    }
    assert_eq!(
        walk(&tree, "cargo gates run slow"),
        unknown("cargo gates run slow")
    );
}

#[test]
fn a_flag_before_the_value_is_skipped_with_the_value_its_option_takes() {
    let tree = fixture_tree();
    for text in [
        "cargo gates run --format json fast",
        "cargo gates run --format=json fast",
        "cargo gates run -v fast",
        "cargo gates run --unknown-switch thorough",
    ] {
        assert_eq!(walk(&tree, text), CitedCommand::Exists, "{text}");
    }
    assert_eq!(
        walk(&tree, "cargo gates run --format json slow"),
        unknown("cargo gates run slow")
    );
    assert_eq!(
        walk(&tree, "cargo gates run -v json"),
        unknown("cargo gates run json"),
        "a switch takes no value"
    );
}

#[test]
fn a_placeholder_where_a_declared_value_belongs_passes() {
    let tree = fixture_tree();
    for text in [
        "cargo gates run <pace>",
        "cargo gates run --format json [pace]",
        "cargo gates run …",
    ] {
        assert_eq!(
            walk(&tree, text),
            CitedCommand::ReachesPlaceholder,
            "{text}"
        );
    }
    assert_eq!(
        walk(&tree, "cargo gates ticket show <id>"),
        CitedCommand::Exists,
        "an argument that declares no values is never judged"
    );
}

#[test]
fn a_group_word_that_names_no_subcommand_is_judged_as_the_group_argument() {
    let tree = fixture_tree();
    for text in ["cargo gates switch status", "cargo gates switch on"] {
        assert_eq!(walk(&tree, text), CitedCommand::Exists, "{text}");
    }
    assert_eq!(
        walk(&tree, "cargo gates switch sideways"),
        unknown("cargo gates switch sideways")
    );
}

#[test]
fn a_value_break_renders_and_a_placeholder_value_counts_as_reached() {
    let text = "# Tool\n\n`cargo gates run slow`, `cargo gates run <pace>` and \
                `cargo gates run fast`.\n";
    let (breaks, totals) = judge_document(text);
    assert_eq!(
        breaks,
        ["apps/tool/README.md:3: cited command does not exist: `cargo gates run slow`"]
    );
    assert!(
        totals.contains(
            "3 judged in live documents — 1 name an existing command, 1 reach a placeholder, 1 \
             name no command"
        ),
        "{totals}"
    );
}

#[test]
fn frozen_records_are_never_judged_by_the_rule() {
    let mut fixture = FixtureCheckout::new("command-citations-frozen");
    let stale = "# Record\n\n`cargo gates ticket frobnicate`\n";
    fixture
        .tracked(&format!("{}/record.md", FROZEN_FOLDERS[0]), stale)
        .tracked("apps/tool/README.md", stale);
    let tree = fixture_tree();
    let mut rules: Vec<Box<dyn DocumentRule + '_>> = vec![Box::new(CommandCitations::new(&tree))];
    let run = judge_gate(
        fixture.root(),
        Ok(fixture.tree()),
        &GateRequest::default(),
        BreakListing::Every,
        &mut rules,
    );
    assert_eq!(outcome_counts(&run), (1, 1, 0));
    assert!(failures(&run)[0].starts_with("FAIL: apps/tool/README.md: 1 break(s)"));
    assert!(
        run.totals
            .contains(&"    cited command does not exist: 1".to_string())
    );
}
