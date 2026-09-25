use super::*;

#[test]
fn list_items_follow_the_commonmark_markers() {
    assert_eq!(
        list_item("- item"),
        Some(ListItem {
            content_offset: 2,
            interrupts_paragraph: true
        })
    );
    assert_eq!(
        list_item("12. step"),
        Some(ListItem {
            content_offset: 4,
            interrupts_paragraph: false
        })
    );
    assert_eq!(
        list_item("1) step").map(|item| item.interrupts_paragraph),
        Some(true)
    );
    assert_eq!(list_item("-no space"), None);
    assert_eq!(list_item("2024 was a year"), None);
    assert_eq!(
        list_item("-      code").map(|item| item.content_offset),
        Some(2),
        "five or more spaces leave the item's content one column past the marker"
    );
}

#[test]
fn atx_headings_drop_their_closing_sequence() {
    assert_eq!(atx_heading("# Title #"), Some("Title"));
    assert_eq!(atx_heading("## Closing##"), Some("Closing##"));
    assert_eq!(atx_heading("### ###"), Some(""));
    assert_eq!(atx_heading("#hashtag"), None);
    assert_eq!(atx_heading("####### seven"), None);
}

#[test]
fn a_line_that_opens_a_block_cannot_continue_a_paragraph() {
    for opener in [
        "- item",
        "1. step",
        "---",
        "===",
        "***",
        "# Heading",
        "```rust",
        "<!-- note",
    ] {
        assert!(opens_block(opener), "{opener}");
    }
    for text in ["plain words", "2024. was a year", "-dash", "#hashtag"] {
        assert!(!opens_block(text), "{text}");
    }
    assert!(is_thematic_break("* * *"));
    assert!(!is_thematic_break("--"));
    assert!(is_setext_underline("=="));
    assert!(!is_setext_underline("=-="));
}

#[test]
fn front_matter_blockquotes_and_tabs_are_read_off_the_line() {
    assert_eq!(front_matter_end(&["---", "name: x", "---", "# Body"]), 3);
    assert_eq!(front_matter_end(&["---", "never closed"]), 0);
    assert_eq!(front_matter_end(&["# Body"]), 0);
    assert_eq!(strip_blockquote("> > nested quote"), "nested quote");
    assert_eq!(
        strip_blockquote("    > code, not a quote"),
        "    > code, not a quote"
    );
    assert_eq!(expand_leading_tabs("\t- item\tx"), "    - item\tx");
    assert_eq!(expand_leading_tabs("  \tx"), "    x");
    assert_eq!(leading_spaces("   x"), 3);
}
