use super::*;

#[test]
fn punctuation_and_case_fold_away() {
    assert_eq!(spoken("Rebecca!?"), vec!["rebecca"]);
    assert_eq!(spoken("don't,"), vec!["don't"]);
    assert!(spoken("—").is_empty());
}

#[test]
fn accents_drop_and_hyphens_split() {
    assert_eq!(spoken("Señor"), vec!["senor"]);
    assert_eq!(spoken("Flame-Flame"), vec!["flame", "flame"]);
}

#[test]
fn numbers_are_read_out() {
    assert_eq!(spoken("15"), vec!["fifteen"]);
    assert_eq!(spoken("2024"), vec!["two", "thousand", "twenty", "four"]);
    assert_eq!(spoken("100%"), vec!["one", "hundred", "percent"]);
}
