use super::*;

#[test]
fn one_search_filters_both_lists_and_showing_a_call_opens_it() {
    let mut console = LogConsole::default();
    assert_eq!(console.view, ConsoleView::Activity);
    console.set_search("  Batch ".into());
    assert_eq!(console.search(), "  Batch ");
    console.show_call("7-3".into());
    assert_eq!(console.view, ConsoleView::Calls);
    assert!(
        console.calls.selected().is_none(),
        "the call is not kept yet"
    );
}
