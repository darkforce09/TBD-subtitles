use super::*;

#[test]
fn own_ticks_sum_user_system_and_reaped_children() {
    let stat = "4242 (visual (bench)) R 1 4242 4242 0 -1 4194304 100 0 0 0 \
                700 50 30 20 20 0 9 0 12345 1000 200";
    assert_eq!(own_ticks(stat), Some(800));
}

#[test]
fn a_truncated_stat_line_has_no_ticks() {
    assert_eq!(own_ticks("4242 (bench) R 1 4242"), None);
    assert_eq!(own_ticks("no parenthesis"), None);
}

#[test]
fn this_process_has_a_tick_count() {
    assert!(own_ticks_now().is_some());
}
