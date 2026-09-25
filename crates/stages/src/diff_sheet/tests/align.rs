use super::*;

fn words(text: &str) -> Vec<String> {
    normalise_all(text.split_whitespace())
}

#[test]
fn case_punctuation_and_number_words_fold() {
    assert_eq!(normalise("Birdcage,"), "birdcage");
    assert_eq!(normalise("'Twas"), "twas");
    assert_eq!(normalise("don’t"), "don't");
    assert_eq!(normalise("Three"), "3");
    assert_eq!(normalise("--"), "");
}

#[test]
fn identical_lists_align_as_matches() {
    let a = words("Law, the Birdcage is closing!");
    let steps = align(&a, &a);
    assert!(steps.iter().all(|s| matches!(s, Step::Match(..))));
    assert_eq!(errors(&a, &a).rate(), 0.0);
}

#[test]
fn a_split_word_is_one_substitution_and_one_insertion() {
    let a = words("the Birdcage is closing");
    let b = words("the bird cage is closing in");
    let e = errors(&a, &b);
    assert_eq!((e.substitutions, e.deletions, e.insertions), (1, 0, 2));
    assert_eq!(e.reference_words, 4);
}

#[test]
fn every_word_of_both_lists_appears_once_in_order() {
    let a = words("a b c d e f");
    let b = words("x b c e f g h");
    let steps = align(&a, &b);
    let mut seen_a = Vec::new();
    let mut seen_b = Vec::new();
    for step in steps {
        match step {
            Step::Match(i, j) | Step::Substitute(i, j) => {
                seen_a.push(i);
                seen_b.push(j);
            }
            Step::Delete(i) => seen_a.push(i),
            Step::Insert(j) => seen_b.push(j),
        }
    }
    assert_eq!(seen_a, (0..a.len()).collect::<Vec<_>>());
    assert_eq!(seen_b, (0..b.len()).collect::<Vec<_>>());
}
