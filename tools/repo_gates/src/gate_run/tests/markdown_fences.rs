use super::*;

#[test]
fn backtick_and_tilde_runs_of_three_or_more_open_a_fence() {
    let (fence, info) = opening("```text").expect("a backtick fence");
    assert_eq!(info, "text");
    assert!(closes(fence, "```"));
    let (fence, info) = opening("~~~~ rust  ").expect("a tilde fence");
    assert_eq!(info, "rust");
    assert!(closes(fence, "~~~~~"));
    assert!(
        !closes(fence, "~~~"),
        "a shorter run does not close a longer fence"
    );
    assert!(
        !closes(fence, "```"),
        "the other character does not close a fence"
    );
}

#[test]
fn short_runs_deep_indentation_and_backticks_in_the_info_string_open_nothing() {
    assert!(opening("``text").is_none());
    assert!(
        opening("    ```text").is_none(),
        "four spaces make an indented code block"
    );
    assert!(
        opening("   ```text").is_some(),
        "three spaces still open a fence"
    );
    assert!(opening("```te`xt").is_none());
    assert!(
        opening("~~~te`xt").is_some(),
        "a tilde fence may carry a backtick"
    );
    assert!(opening("text").is_none());
}

#[test]
fn a_closing_line_carries_nothing_but_the_run() {
    let (fence, _) = opening("```").expect("a bare fence");
    assert!(closes(fence, "```   "));
    assert!(!closes(fence, "``` text"));
    assert!(!closes(fence, "    ```"));
}
