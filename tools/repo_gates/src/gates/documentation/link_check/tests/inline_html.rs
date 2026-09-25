use super::*;

fn tag(text: &str) -> Option<(usize, Vec<String>)> {
    let chars: Vec<char> = text.chars().collect();
    html_tag(&chars, 0)
}

#[test]
fn an_anchor_tag_declares_its_id_and_name() {
    assert_eq!(
        tag("<a id=\"setup\" name='alias' class=x>"),
        Some((34, vec!["setup".to_string(), "alias".to_string()]))
    );
    assert_eq!(
        tag("<A ID=\"upper\"/>"),
        Some((14, vec!["upper".to_string()]))
    );
    assert_eq!(
        tag("<a id=\"\">"),
        Some((8, Vec::new())),
        "an empty id names nothing"
    );
}

#[test]
fn other_tags_end_where_they_close_and_declare_nothing() {
    assert_eq!(tag("<span id=\"x\">"), Some((12, Vec::new())));
    assert_eq!(tag("</a>"), Some((3, Vec::new())));
    assert_eq!(tag("<br/>"), Some((4, Vec::new())));
    assert_eq!(tag("<abbr title=\"a > b\">"), Some((19, Vec::new())));
}

#[test]
fn text_that_is_no_tag_is_refused() {
    for text in [
        "< a>",
        "<1a>",
        "<a id=>",
        "<a id=\"open>",
        "<a",
        "</a id=\"x\">",
    ] {
        assert_eq!(tag(text), None, "{text}");
    }
}
