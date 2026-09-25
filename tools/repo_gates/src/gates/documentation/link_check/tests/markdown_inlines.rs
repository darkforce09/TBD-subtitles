use super::*;

fn inline(text: &str) -> InlineScan {
    inline_with(text, &[])
}

fn inline_with(text: &str, labels: &[&str]) -> InlineScan {
    let labels: BTreeSet<String> = labels.iter().map(|label| normalise_label(label)).collect();
    let lines: Vec<(usize, String)> = text
        .lines()
        .enumerate()
        .map(|(index, line)| (index + 1, line.to_string()))
        .collect();
    InlineScan::run(&lines, &labels)
}

fn destinations(found: &InlineScan) -> Vec<&str> {
    found
        .links
        .iter()
        .map(|link| link.destination.as_str())
        .collect()
}

#[test]
fn an_image_inside_a_link_is_found_with_the_link() {
    let found = inline("[![badge](badge.svg)](status.md)");
    assert_eq!(destinations(&found), ["badge.svg", "status.md"]);
}

#[test]
fn links_do_not_nest() {
    let found = inline("[outer [inner](inner.md)](outer.md)");
    assert_eq!(destinations(&found), ["inner.md"]);
}

#[test]
fn a_failed_inline_tail_falls_back_to_a_shortcut_reference() {
    let found = inline_with("[plan] (not a tail) and [plan](<broken)", &["plan"]);
    assert!(found.links.is_empty());
    assert!(found.undefined_references.is_empty());
}

#[test]
fn a_reference_label_matches_without_regard_to_case_or_spacing() {
    let found = inline_with("[a][The   Plan] and [b][the plan]", &["the plan"]);
    assert!(found.undefined_references.is_empty());
    assert_eq!(normalise_label("  The\n Plan "), "the plan");
}

#[test]
fn a_link_with_empty_text_before_an_undefined_label_is_plain_text() {
    let found = inline("Schema {points[][2] minItems 2} and [outer [][2]](outer.md)");
    assert!(found.undefined_references.is_empty());
    assert_eq!(
        destinations(&found),
        ["outer.md"],
        "literal brackets inside a link text leave the link standing"
    );
    assert_eq!(
        found.rendered,
        "Schema {points[][2] minItems 2} and [outer [][2]]"
    );
}

#[test]
fn a_link_with_empty_text_before_a_defined_label_is_a_reference_link() {
    let found = inline_with("[outer [][2]](outer.md)", &["2"]);
    assert!(found.undefined_references.is_empty());
    assert!(
        found.links.is_empty(),
        "the empty-text link closes first, and a link may not contain another link"
    );
}

#[test]
fn an_image_or_a_link_with_text_reports_its_undefined_label() {
    let found = inline("![][logo] [text][gone] [text][] ![alt][]");
    let labels: Vec<&str> = found
        .undefined_references
        .iter()
        .map(|reference| reference.label.as_str())
        .collect();
    assert_eq!(labels, ["logo", "gone", "text", "alt"]);
}

#[test]
fn a_code_span_binds_before_brackets_and_autolinks() {
    let found = inline("`[a](x.md)` then `` `<https://x>` `` then [b](y.md)");
    assert_eq!(destinations(&found), ["y.md"]);
    let spans: Vec<&str> = found
        .code_spans
        .iter()
        .map(|span| span.text.as_str())
        .collect();
    assert_eq!(spans, ["[a](x.md)", "`<https://x>`"]);
}

#[test]
fn a_code_span_may_wrap_across_lines() {
    let found = inline("A `wrapped\n[a](x.md)` span and [b](y.md)");
    assert_eq!(destinations(&found), ["y.md"]);
    assert_eq!(found.code_spans[0].text, "wrapped [a](x.md)");
    assert_eq!(found.code_spans[0].line, 1);
}

#[test]
fn autolinks_need_a_scheme_and_no_spaces() {
    let found = inline("<https://a.example/x> <mailto:me@example.com> <not a link> <./path.md>");
    assert_eq!(
        destinations(&found),
        ["https://a.example/x", "mailto:me@example.com",]
    );
}

#[test]
fn anchor_tags_declare_ids_and_names() {
    let found =
        inline("<a id=\"first\"></a><A NAME='second'></A><a href=\"x\">x</a><span id=\"no\">");
    assert_eq!(found.anchors, ["first", "second"]);
}

#[test]
fn rendered_text_keeps_code_and_link_text_and_drops_destinations_and_images() {
    let found = inline("The `cargo xtask` [guide](guide.md) ![icon](icon.png)and <b>bold</b>");
    assert_eq!(found.rendered, "The cargo xtask [guide] and bold");
}

#[test]
fn rendered_text_drops_paired_emphasis_underscores_and_keeps_the_rest() {
    assert_eq!(
        inline("_emphasis_ and __strong__").rendered,
        "emphasis and strong"
    );
    assert_eq!(inline("snake_case_name").rendered, "snake_case_name");
    assert_eq!(
        inline("a _dangling underscore").rendered,
        "a _dangling underscore"
    );
    assert_eq!(inline("`code_span` _x_").rendered, "code_span x");
}

#[test]
fn rendered_text_decodes_character_references() {
    assert_eq!(inline("Q&amp;A &#233;t&#xE9; &").rendered, "Q&A été &");
}

#[test]
fn an_escaped_character_is_literal() {
    let found = inline("\\[x\\](y.md) \\`z\\`");
    assert!(found.links.is_empty());
    assert_eq!(found.rendered, "[x](y.md) `z`");
}
