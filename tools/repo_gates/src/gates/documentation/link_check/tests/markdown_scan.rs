use super::*;

fn destinations(document: &ScannedDocument) -> Vec<(usize, &str)> {
    document
        .links
        .iter()
        .map(|link| (link.line, link.destination.as_str()))
        .collect()
}

fn headings(document: &ScannedDocument) -> Vec<(usize, &str)> {
    document
        .headings
        .iter()
        .map(|heading| (heading.line, heading.text.as_str()))
        .collect()
}

#[test]
fn inline_links_images_and_autolinks_carry_their_destination_line() {
    let document = scan(
        "# Title\n\nSee [the plan](/documentation/plan.md \"Plan\") and ![map](images/map.png).\n\
         A <https://example.com/x> autolink and [spaced](<folder/my file.md>).\n",
    );
    assert_eq!(
        destinations(&document),
        [
            (3, "/documentation/plan.md"),
            (3, "images/map.png"),
            (4, "https://example.com/x"),
            (4, "folder/my file.md"),
        ]
    );
}

#[test]
fn a_link_whose_text_wraps_reports_the_line_its_destination_starts_on() {
    let document = scan("Read [the long\nwrapped title](target.md) now.\n");
    assert_eq!(destinations(&document), [(2, "target.md")]);
}

#[test]
fn references_resolve_against_definitions_anywhere_in_the_document() {
    let document = scan(
        "Use [full][Plan], [Plan][], [plan] and [missing][nowhere] or [gone][].\n\
         Shortcut [not a reference] and a task list [ ] stay text.\n\
         \n\
         [plan]: /documentation/plan.md \"The plan\"\n\
         [PLAN]: /elsewhere.md\n",
    );
    assert_eq!(
        destinations(&document),
        [(4, "/documentation/plan.md")],
        "a definition is judged once, at its line; the first definition of a label wins"
    );
    let undefined: Vec<(usize, &str)> = document
        .undefined_references
        .iter()
        .map(|reference| (reference.line, reference.label.as_str()))
        .collect();
    assert_eq!(undefined, [(1, "nowhere"), (1, "gone")]);
}

#[test]
fn a_link_with_empty_text_is_judged_through_its_definition_alone() {
    let undefined = scan("1. Schema {id, points[][2] minItems 2, label?}; golden updated.\n");
    assert!(undefined.undefined_references.is_empty());
    assert!(undefined.links.is_empty());
    let defined = scan("See [][2].\n\n[2]: /documentation/two.md\n");
    assert!(defined.undefined_references.is_empty());
    assert_eq!(
        destinations(&defined),
        [(3, "/documentation/two.md")],
        "a defined label's destination is judged at its definition, as for any reference"
    );
}

#[test]
fn footnotes_are_not_references_but_their_text_is_scanned() {
    let document = scan("Claim[^1].\n\n[^1]: See [the source](source.md).\n");
    assert_eq!(destinations(&document), [(3, "source.md")]);
    assert!(document.undefined_references.is_empty());
}

#[test]
fn code_spans_fences_and_indented_code_hold_no_links() {
    let document = scan(
        "Inline `[a](code.md)` and ``[b](`tick`.md)`` stay code; [real](real.md).\n\
         \n\
         ```text\n\
         [c](fenced.md)\n\
         ```\n\
         \n\
         ~~~~\n\
         [d](tilde.md)\n\
         ~~~~\n\
         \n\
         \x20   [e](indented.md)\n\
         \n\
         After [f](after.md).\n",
    );
    assert_eq!(destinations(&document), [(1, "real.md"), (13, "after.md")]);
    let spans: Vec<&str> = document
        .code_spans
        .iter()
        .map(|span| span.text.as_str())
        .collect();
    assert_eq!(spans, ["[a](code.md)", "[b](`tick`.md)"]);
    let blocks: Vec<(usize, &str, Vec<&str>)> = document
        .code_blocks
        .iter()
        .map(|block| {
            (
                block.line,
                block.info.as_str(),
                block.lines.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    assert_eq!(
        blocks,
        [
            (3, "text", vec!["[c](fenced.md)"]),
            (7, "", vec!["[d](tilde.md)"])
        ]
    );
}

#[test]
fn a_fence_inside_a_list_item_is_found_at_the_item_content_column() {
    let document = scan(
        "1. Run this:\n\
         \n\
         \x20   ```bash\n\
         \x20   [x](inside.md)\n\
         \x20   ```\n\
         \n\
         \x20   Then read [the result](result.md).\n\
         - item\n\
         \x20   - nested [link](nested.md)\n",
    );
    assert_eq!(
        destinations(&document),
        [(7, "result.md"), (9, "nested.md")]
    );
    assert_eq!(document.code_blocks.len(), 1);
}

#[test]
fn an_unclosed_code_span_is_literal_backticks() {
    let document = scan("A stray ` backtick before [a link](target.md).\n");
    assert_eq!(destinations(&document), [(1, "target.md")]);
}

#[test]
fn html_comments_and_front_matter_hold_no_links() {
    let document = scan(
        "---\ntitle: [x](front.md)\n---\n\
         <!-- a comment\n[y](comment.md)\n-->\n\
         Text <!-- [z](inline.md) --> with [w](kept.md).\n",
    );
    assert_eq!(destinations(&document), [(7, "kept.md")]);
}

#[test]
fn escaped_brackets_open_no_link() {
    let document = scan("Not \\[a link](nowhere.md), but [one](here.md\\(1\\)).\n");
    assert_eq!(destinations(&document), [(1, "here.md(1)")]);
}

#[test]
fn blockquotes_and_table_rows_are_scanned() {
    let document =
        scan("> Quoted [q](quoted.md).\n\n| a | b |\n|---|---|\n| [t](table.md) | x |\n");
    assert_eq!(destinations(&document), [(1, "quoted.md"), (5, "table.md")]);
}

#[test]
fn atx_and_setext_headings_are_read_and_code_never_is() {
    let document = scan(
        "# Title #\n\nIntro\n===\n\nSection text\n---\n\n```md\n# Not a heading\n```\n\n\
         - list item\n---\n\n    # indented code\n#hashtag\n## Closing##\n",
    );
    assert_eq!(
        headings(&document),
        [
            (1, "Title"),
            (3, "Intro"),
            (6, "Section text"),
            (18, "Closing##")
        ]
    );
}

#[test]
fn a_heading_s_text_is_rendered_for_its_anchor() {
    let document = scan("## The `link-check` gate: [see](x.md) <a id=\"gate\"></a>\n");
    assert_eq!(headings(&document), [(1, "The link-check gate: [see] ")]);
    assert_eq!(document.explicit_anchors, ["gate"]);
}
