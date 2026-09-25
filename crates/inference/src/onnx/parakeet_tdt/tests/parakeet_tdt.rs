use super::*;

fn word(text: &str, start_s: f64) -> TimedWord {
    TimedWord {
        text: text.into(),
        start_s,
        end_s: start_s + 0.1,
        confidence: None,
    }
}

#[test]
fn punctuation_tokens_join_the_word_before() {
    let words = attach_punctuation([
        word("Hello", 0.0),
        word(",", 0.1),
        word("Rebecca", 0.2),
        word("!", 0.3),
        word("?", 0.3),
    ]);
    let texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
    assert_eq!(texts, vec!["Hello,", "Rebecca!?"]);
    assert_eq!(words[1].start_s, 0.2);
}

#[test]
fn leading_punctuation_is_kept_as_a_word() {
    let words = attach_punctuation([word("...", 0.0), word("Law", 0.5)]);
    assert_eq!(words.len(), 2);
}
