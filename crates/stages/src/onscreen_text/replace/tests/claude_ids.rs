use super::found_by_claude;

#[test]
fn only_ids_ending_in_c_and_a_number_are_found_by_claude() {
    assert!(found_by_claude("text-001565-c1"));
    assert!(found_by_claude("text-000574-c12"));
    assert!(!found_by_claude("text-000584"));
    assert!(!found_by_claude("text-000584-c"));
    assert!(!found_by_claude("text-000584-cx"));
    assert!(!found_by_claude("-c1"));
}
