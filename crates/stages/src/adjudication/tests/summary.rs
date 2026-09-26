use super::*;

#[test]
fn names_count_as_whole_words() {
    assert_eq!(count_word("Law, Lawson and Law!", "Law"), 2);
    assert_eq!(count_word("Straw Hat Pirates", "Straw Hat"), 1);
}
