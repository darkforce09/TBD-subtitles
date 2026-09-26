use super::*;

fn split(text: &str) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    layout(&words).expect("fits")
}

#[test]
fn short_text_stays_on_one_line() {
    assert_eq!(
        split("Law, the Birdcage is closing in!"),
        vec!["Law, the Birdcage is closing in!"]
    );
    let exactly = "a".repeat(MAX_LINE);
    assert_eq!(layout(&[exactly.as_str()]), Some(vec![exactly.clone()]));
}

#[test]
fn long_text_breaks_after_punctuation() {
    let lines = split("I told you already, Doflamingo is a man who never forgives.");
    assert_eq!(
        lines,
        vec![
            "I told you already,",
            "Doflamingo is a man who never forgives."
        ]
    );
}

#[test]
fn an_article_stays_with_its_noun_and_a_name_stays_whole() {
    let lines = split("We have to get to the top of the tower before Trafalgar Law gets there");
    for pair in lines.windows(2) {
        assert!(!pair[0].ends_with(" the"), "{lines:?}");
        assert!(!pair[0].ends_with("Trafalgar"), "{lines:?}");
    }
    let names = split("Everybody listen, the one who stands there is Trafalgar Law himself");
    assert!(!names[0].ends_with("Trafalgar"), "{names:?}");
}

#[test]
fn the_bottom_line_is_the_longer_one_and_the_top_is_never_one_or_two_words() {
    let lines = split("so you really think that we can win this fight against all of them today");
    assert_eq!(lines.len(), 2);
    assert!(
        lines[0].chars().count() <= lines[1].chars().count() + 6,
        "{lines:?}"
    );
    assert!(lines[0].split_whitespace().count() > 2, "{lines:?}");
}

#[test]
fn text_over_two_lines_does_not_fit() {
    let words: Vec<&str> = std::iter::repeat_n("abcdefghij", 9).collect();
    assert!(!fits(&words));
    assert!(fits(&["one", "two"]));
}
