use super::*;

#[test]
fn the_built_in_glossary_parses_and_holds_the_main_names() {
    let terms = one_piece();
    assert!(terms.len() >= 50, "{} terms", terms.len());
    for name in [
        "Luffy",
        "Doflamingo",
        "Trafalgar Law",
        "Kin'emon",
        "Flame-Flame Fruit",
    ] {
        assert!(terms.iter().any(|t| t == name), "{name} missing");
    }
}

#[test]
fn a_glossary_file_must_be_an_array_of_strings() {
    assert_eq!(parse(r#"[" Riku ", ""]"#), Ok(vec!["Riku".to_string()]));
    assert!(parse(r#"{"names": []}"#).is_err());
}
