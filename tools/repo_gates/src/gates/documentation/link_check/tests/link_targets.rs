use super::super::Break;
use super::*;
use crate::gate_run::fixture_checkout::FixtureCheckout;

/// Judge `document` (written into `fixture` as tracked) with the link rule, and return every
/// break as it renders plus how many checks did not run.
fn judge_document(
    fixture: &mut FixtureCheckout,
    document: &str,
    text: &str,
) -> (Vec<String>, usize) {
    fixture.tracked(document, text);
    let tree = fixture.tree();
    let context = RuleContext {
        repo_root: fixture.root(),
        tree: &tree,
    };
    let scanned = scan(text);
    let mut rule = LinkTargets::new();
    let mut findings = RuleFindings::default();
    rule.judge(
        &JudgedDocument {
            path: document,
            scan: &scanned,
        },
        &context,
        &mut findings,
    );
    rule.finish(&context, &mut findings);
    let breaks = findings.breaks.iter().map(Break::render).collect();
    (breaks, findings.not_run.len())
}

fn checkout(tag: &str) -> FixtureCheckout {
    let mut fixture = FixtureCheckout::new(&format!("link-targets-{tag}"));
    fixture
        .tracked(
            "documentation/guide.md",
            "# Guide\n\n## Setup steps\n\nText.\n",
        )
        .tracked("apps/tool/src/main.rs", "fn main() {}\n// two\n// three\n")
        .tracked("apps/tool/README.md", "# Tool\n");
    fixture
}

#[test]
fn a_checkout_path_must_name_a_tracked_file_or_folder() {
    let mut fixture = checkout("paths");
    fixture.untracked("documentation/draft.md", "# Draft\n");
    let text = "[ok](guide.md) [folder](/apps/tool/) [root](/)\n\
                [gone](missing.md) [draft](draft.md)\n[out](../../outside.md)\n";
    let (breaks, not_run) = judge_document(&mut fixture, "documentation/index.md", text);
    assert_eq!(
        breaks,
        [
            "documentation/index.md:2: missing target: `missing.md` resolves to \
             `documentation/missing.md`, which is no tracked file or folder",
            "documentation/index.md:2: missing target: `draft.md` resolves to \
             `documentation/draft.md`, which is no tracked file or folder",
            "documentation/index.md:3: escapes repository: `../../outside.md` climbs above the \
             repository root",
        ]
    );
    assert_eq!(not_run, 0);
}

#[test]
fn a_fragment_must_match_a_heading_or_fit_a_line_anchor() {
    let mut fixture = checkout("fragments");
    let text = "[a](guide.md#setup-steps) [b](guide.md#nope) [c](#local) [d](#elsewhere)\n\
                [e](/apps/tool/src/main.rs#L2-L3) [f](/apps/tool/src/main.rs#L4)\n\
                [g](/apps/tool/src/main.rs#main) [h](/apps/tool#x) [i](guide.md?plain=1#L5)\n\
                [j](guide.md#L1)\n\n## Local\n";
    let (breaks, _) = judge_document(&mut fixture, "documentation/index.md", text);
    assert_eq!(
        breaks,
        [
            "documentation/index.md:1: missing anchor: `guide.md#nope`: \
             `documentation/guide.md` has no heading or anchor `nope`",
            "documentation/index.md:1: missing anchor: `#elsewhere`: this document has no \
             heading or anchor `elsewhere`",
            "documentation/index.md:2: line anchor out of range: \
             `/apps/tool/src/main.rs#L4`: the file has 3 line(s)",
            "documentation/index.md:3: missing anchor: `/apps/tool/src/main.rs#main`: \
             `apps/tool/src/main.rs` is not rendered Markdown, so `main` matches nothing; only a \
             #L<n> or #L<n>-L<m> line anchor applies",
            "documentation/index.md:3: missing anchor: `/apps/tool#x`: `apps/tool` is a \
             folder, which has no anchors",
            "documentation/index.md:4: missing anchor: `guide.md#L1`: \
             `documentation/guide.md` has no heading or anchor `L1`",
        ]
    );
}

#[test]
fn an_encoded_fragment_decodes_before_it_is_matched() {
    let mut fixture = checkout("encoded");
    let text = "# Überblick\n\n[a](#%C3%BCberblick) [b](#überblick)\n";
    let (breaks, _) = judge_document(&mut fixture, "documentation/index.md", text);
    assert_eq!(breaks, Vec::<String>::new());
}

#[test]
fn an_undefined_reference_breaks() {
    let mut fixture = checkout("references");
    let (breaks, _) = judge_document(&mut fixture, "README.md", "[a][nowhere]\n");
    assert_eq!(
        breaks,
        [
            "README.md:1: undefined reference: `[nowhere]` names no reference definition in this \
         document"
        ]
    );
}

#[test]
fn an_unreadable_target_did_not_run_once() {
    let mut fixture = checkout("unreadable");
    fixture.listed_only("documentation/lost.md");
    let text = "[a](lost.md#one) [b](lost.md#two) [c](lost.md)\n";
    let (breaks, not_run) = judge_document(&mut fixture, "documentation/index.md", text);
    assert_eq!(breaks, Vec::<String>::new());
    assert_eq!(not_run, 1, "one verdict for the target, not one per link");
}

#[test]
fn external_links_are_counted_and_never_fetched() {
    let mut fixture = checkout("external");
    let text = "[a](https://example.com) <mailto:x@example.com> [b](guide.md)\n";
    fixture.tracked("documentation/index.md", text);
    let tree = fixture.tree();
    let context = RuleContext {
        repo_root: fixture.root(),
        tree: &tree,
    };
    let mut rule = LinkTargets::new();
    let mut findings = RuleFindings::default();
    let scanned = scan(text);
    let document = JudgedDocument {
        path: "documentation/index.md",
        scan: &scanned,
    };
    rule.judge(&document, &context, &mut findings);
    rule.finish(&context, &mut findings);
    assert!(findings.breaks.is_empty());
    assert_eq!(
        rule.totals(),
        ["  links: 3 judged — 1 into this checkout, 2 external and not fetched"]
    );
}
