use super::{Names, sanitize};

#[test]
fn ids_become_safe_folder_names() {
    assert_eq!(sanitize("text-0001_a"), "text-0001_a");
    assert_eq!(sanitize("a/b..c 文"), "a_b__c__");
    assert_eq!(sanitize(""), "occurrence");
}

#[test]
fn colliding_names_get_a_suffix() {
    let mut names = Names::default();
    assert_eq!(names.claim("a.b"), "a_b");
    assert_eq!(names.claim("a_b"), "a_b-2");
    assert_eq!(names.claim("a/b"), "a_b-3");
    assert_eq!(names.claim("c"), "c");
}
