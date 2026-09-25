use super::*;

const GOOD: &str = "# Media input\n\nWhat it is for.\n\n## Contents\n\n```text\ncrates/x/\n\
                    ## not a heading inside a fence\n```\n\n## How it works\n\nText.\n\n\
                    ## Public surface\n\n- `x`\n\n## Boundaries\n\n- Depends on: a.\n- Used by: \
                    b,\n  wrapped.\n- Rules: c.\n  - nested detail\n\n## Related documentation\n\n\
                    - [Doc](/documentation/x.md) — what.\n";

fn problems(path: &str, text: &str) -> Vec<String> {
    ReadmeSections.problems(path, text.as_bytes())
}

#[test]
fn a_readme_in_the_standard_shape_passes() {
    assert_eq!(problems("crates/x/README.md", GOOD), Vec::<String>::new());
    let documented = format!("**Status:** live\n\n{GOOD}");
    assert_eq!(
        problems("documentation/x/README.md", &documented),
        Vec::<String>::new()
    );
}

#[test]
fn unknown_repeated_and_misordered_sections_fail() {
    let text = GOOD
        .replace("## How it works", "## Notes")
        .replace("## Public surface", "## Contents");
    assert_eq!(
        problems("crates/x/README.md", &text),
        [
            "crates/x/README.md:12: `## Notes` is no README section; finer structure goes in `###`",
            "crates/x/README.md:16: `## Contents` is out of order or repeated",
        ]
    );
}

#[test]
fn the_title_contents_and_boundaries_are_required() {
    let found = problems("crates/x/README.md", "Intro.\n\n## How it works\n");
    assert_eq!(
        found,
        [
            "crates/x/README.md:3: the first heading is the H1 title",
            "crates/x/README.md:1: no `## Contents` section",
            "crates/x/README.md:1: no `## Boundaries` section",
        ]
    );
}

#[test]
fn boundaries_holds_the_three_bullets_in_order() {
    let swapped = GOOD.replace(
        "- Depends on: a.\n- Used by:",
        "- Used by: a.\n- Depends on:",
    );
    assert_eq!(problems("crates/x/README.md", &swapped).len(), 1);
    let extra = GOOD.replace("- Rules: c.", "- Rules: c.\n- Notes: d.");
    assert!(problems("crates/x/README.md", &extra)[0].ends_with("found 4"));
}

#[test]
fn a_code_readme_carries_no_status_line() {
    let text = format!("**Status:** live\n\n{GOOD}");
    assert_eq!(
        problems("crates/x/README.md", &text),
        ["crates/x/README.md:1: a code README carries no status line"]
    );
}

#[test]
fn only_readmes_in_the_span_are_judged() {
    assert!(ReadmeSections.selects("crates/x/src/README.md"));
    assert!(ReadmeSections.selects("documentation/README.md"));
    assert!(!ReadmeSections.selects("README.md"));
    assert!(!ReadmeSections.selects("crates/x/src/tests/README.md"));
}
