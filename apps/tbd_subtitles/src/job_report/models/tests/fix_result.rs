use super::*;

#[test]
fn a_change_reads_as_plain_words() {
    assert_eq!(
        change_words("Heaven dish", "Cavendish"),
        "“Heaven dish” → “Cavendish”"
    );
    assert_eq!(change_words("", "Uh,"), "Added “Uh,”");
    assert_eq!(change_words("So", ""), "Removed “So”");
}
