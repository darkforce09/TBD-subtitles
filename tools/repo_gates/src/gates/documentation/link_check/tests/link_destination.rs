use super::*;

/// The destination of the inline tail after `]` in `text`, whose `(` is its first character.
fn tail(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    inline_tail(&chars, 0).map(|found| found.destination)
}

#[test]
fn a_bare_destination_ends_at_a_space_or_the_unbalanced_parenthesis() {
    assert_eq!(tail("(plain.md)").as_deref(), Some("plain.md"));
    assert_eq!(tail("(a_(b)_c.md)").as_deref(), Some("a_(b)_c.md"));
    assert_eq!(
        tail("(escaped\\)paren.md)").as_deref(),
        Some("escaped)paren.md")
    );
    assert_eq!(
        tail("(two words.md)"),
        None,
        "a bare destination holds no space"
    );
    assert_eq!(tail("(unclosed(paren.md)"), None);
    assert_eq!(tail("()").as_deref(), Some(""));
}

#[test]
fn an_angle_destination_may_hold_spaces_but_no_line_break() {
    assert_eq!(tail("(<my file.md>)").as_deref(), Some("my file.md"));
    assert_eq!(tail("(<a\\>b.md>)").as_deref(), Some("a>b.md"));
    assert_eq!(tail("(<broken\nline.md>)"), None);
    assert_eq!(tail("(<unclosed.md)"), None);
}

#[test]
fn a_title_follows_the_destination_after_whitespace() {
    assert_eq!(tail("(x.md \"Title\")").as_deref(), Some("x.md"));
    assert_eq!(tail("(x.md 'Title')").as_deref(), Some("x.md"));
    assert_eq!(tail("(x.md (Title))").as_deref(), Some("x.md"));
    assert_eq!(tail("(\n  x.md\n  \"Title\"\n)").as_deref(), Some("x.md"));
    assert_eq!(tail("(x.md \"unclosed)"), None);
    assert_eq!(tail("(x.md trailing)"), None);
}

#[test]
fn the_tail_reports_where_the_destination_starts_and_the_parenthesis_closes() {
    let chars: Vec<char> = "(  x.md )".chars().collect();
    assert_eq!(
        inline_tail(&chars, 0),
        Some(InlineTail {
            destination: "x.md".to_string(),
            start: 3,
            end: 8
        })
    );
}

#[test]
fn a_definition_line_names_a_label_and_a_destination() {
    assert_eq!(
        definition("[Plan]: /documentation/plan.md \"The plan\""),
        Some(Definition {
            label: "Plan".to_string(),
            destination: "/documentation/plan.md".to_string()
        })
    );
    assert_eq!(
        definition("[spaced]: <a file.md>").map(|found| found.destination),
        Some("a file.md".to_string())
    );
    assert_eq!(
        definition("[empty]: <>").map(|found| found.destination),
        Some(String::new())
    );
}

#[test]
fn a_line_that_only_looks_like_a_definition_is_not_one() {
    assert_eq!(definition("[^1]: a footnote"), None);
    assert_eq!(definition("[label]: /x.md trailing words"), None);
    assert_eq!(definition("[label]:"), None);
    assert_eq!(definition("[label] : /x.md"), None);
    assert_eq!(definition("[ ]: /x.md"), None);
    assert_eq!(definition("text [label]: /x.md"), None);
}

#[test]
fn a_reference_label_ends_at_its_bracket_and_never_nests() {
    let chars: Vec<char> = "[a \\] b]".chars().collect();
    assert_eq!(reference_label(&chars, 0), Some(("a \\] b".to_string(), 7)));
    let nested: Vec<char> = "[a [b] c]".chars().collect();
    assert_eq!(reference_label(&nested, 0), None);
    let long: Vec<char> = format!("[{}]", "x".repeat(1000)).chars().collect();
    assert_eq!(reference_label(&long, 0), None);
}
