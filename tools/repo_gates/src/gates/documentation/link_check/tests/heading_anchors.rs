use super::super::markdown_scan::scan;
use super::*;

fn anchors_of(text: &str) -> Vec<String> {
    document_anchors(&scan(text)).into_iter().collect()
}

#[test]
fn a_slug_is_lowercase_with_punctuation_dropped_and_spaces_hyphenated() {
    assert_eq!(slug("Getting Started"), "getting-started");
    assert_eq!(slug("What's new? (2026 edition)"), "whats-new-2026-edition");
    assert_eq!(slug("Boundaries & Rules"), "boundaries--rules");
    assert_eq!(
        slug("snake_case and kebab-case"),
        "snake_case-and-kebab-case"
    );
    assert_eq!(slug("  Two  spaces "), "--two--spaces-");
}

#[test]
fn a_slug_keeps_letters_of_every_script_and_drops_emoji() {
    assert_eq!(slug("Übersicht der Ränge"), "übersicht-der-ränge");
    assert_eq!(slug("Карта мира"), "карта-мира");
    assert_eq!(slug("🚀 Features"), "-features");
    assert_eq!(slug("Deploy ✅ done"), "deploy--done");
}

#[test]
fn repeated_headings_are_numbered_in_document_order() {
    assert_eq!(
        anchors_of("# Setup\n## Setup\n### Setup\n## Setup-1\n"),
        ["setup", "setup-1", "setup-1-1", "setup-2"]
    );
}

#[test]
fn setext_headings_count_and_fenced_ones_do_not() {
    assert_eq!(
        anchors_of("Overview\n========\n\nDetails\n-------\n\n```md\n# Hidden\n```\n"),
        ["details", "overview"]
    );
}

#[test]
fn heading_markup_renders_before_the_slug_is_taken() {
    assert_eq!(
        anchors_of(
            "## The `link-check` gate\n## [Linked](x.md) heading\n## _Emphasis_ and **bold**\n"
        ),
        ["emphasis-and-bold", "linked-heading", "the-link-check-gate"]
    );
}

#[test]
fn explicit_anchors_join_the_heading_anchors() {
    assert_eq!(
        anchors_of("<a id=\"top-of-page\"></a>\n\n# Title\n\nText <a name=\"Mixed_Case\"></a>.\n"),
        ["Mixed_Case", "title", "top-of-page"]
    );
}
