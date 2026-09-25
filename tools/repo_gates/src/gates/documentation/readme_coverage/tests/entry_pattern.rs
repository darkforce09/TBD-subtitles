use super::*;

fn pattern(name: &str) -> EntryPattern {
    EntryPattern::parse(name).expect("a valid entry")
}

#[test]
fn a_name_matches_only_itself() {
    let name = pattern("mod.rs");
    assert!(matches!(name, EntryPattern::Name(_)));
    assert!(name.matches("mod.rs"));
    assert!(!name.matches("mod.rs.bak"));
    assert!(!name.matches("Mod.rs"));
    assert!(name.is_name("mod.rs"));
}

#[test]
fn a_star_matches_any_run_including_a_leading_dot() {
    let glob = pattern("*.rs");
    assert!(glob.matches("mod.rs"));
    assert!(glob.matches(".rs"));
    assert!(glob.matches(".hidden.rs"));
    assert!(
        !glob.matches("mod.rs.bak"),
        "a glob matches whole names only"
    );
    assert!(pattern("*").matches(".gitignore"));
    assert!(!glob.is_name("mod.rs"), "a glob is never a name");
}

#[test]
fn a_question_mark_matches_exactly_one_character() {
    let glob = pattern("t?.md");
    assert!(glob.matches("t1.md"));
    assert!(glob.matches("té.md"));
    assert!(!glob.matches("t.md"));
    assert!(!glob.matches("t12.md"));
}

#[test]
fn a_class_matches_one_member_a_range_or_a_negation() {
    let set = pattern("[abc].rs");
    assert!(set.matches("b.rs"));
    assert!(!set.matches("d.rs"));
    let range = pattern("v[0-9].json");
    assert!(range.matches("v7.json"));
    assert!(!range.matches("vx.json"));
    for negated in ["[!a]x", "[^a]x"] {
        let glob = pattern(negated);
        assert!(glob.matches("bx"), "{negated}");
        assert!(!glob.matches("ax"), "{negated}");
    }
    let bracket = pattern("[]a]");
    assert!(bracket.matches("]"), "a `]` in first place is a member");
    assert!(bracket.matches("a"));
    let dash = pattern("[a-]");
    assert!(
        dash.matches("-"),
        "a `-` before the closing `]` is a member"
    );
}

#[test]
fn braces_choose_one_alternative_and_nest() {
    let glob = pattern("{mod,lib}.rs");
    assert!(glob.matches("mod.rs"));
    assert!(glob.matches("lib.rs"));
    assert!(!glob.matches("main.rs"));
    let nested = pattern("{a,{b,c}*}.md");
    assert!(nested.matches("a.md"));
    assert!(nested.matches("cat.md"));
    assert!(!nested.matches("d.md"));
    let plain_comma = pattern("a,b*");
    assert!(
        plain_comma.matches("a,bc"),
        "a comma outside braces is literal"
    );
}

#[test]
fn regular_expression_characters_are_literal() {
    let glob = pattern("a+b(1)*.rs");
    assert!(glob.matches("a+b(1)x.rs"));
    assert!(!glob.matches("aab1x.rs"));
}

#[test]
fn a_malformed_glob_is_refused_with_its_reason() {
    for (glob, reason) in [
        ("[ab", "has an unclosed `[`"),
        ("{a,b", "has an unclosed `{`"),
    ] {
        let refused = EntryPattern::parse(glob).expect_err(glob);
        assert_eq!(refused, reason);
    }
    let reversed = EntryPattern::parse("[z-a]").expect_err("a reversed range");
    assert!(reversed.starts_with("is not a valid glob: "), "{reversed}");
}
