use super::*;

fn result(seq: u64) -> ScreenResult {
    ScreenResult {
        seq,
        regions: Vec::new(),
    }
}

#[test]
fn results_are_released_in_sequence_order() {
    let mut order = InOrder::new(0);
    order.push(result(2)).unwrap();
    order.push(result(1)).unwrap();
    assert!(order.pop().is_none());
    assert_eq!(order.waiting(), 2);
    order.push(result(0)).unwrap();
    let released: Vec<u64> = std::iter::from_fn(|| order.pop()).map(|r| r.seq).collect();
    assert_eq!(released, [0, 1, 2]);
    assert_eq!(order.waiting(), 0);
}

#[test]
fn a_number_twice_is_refused() {
    let mut order = InOrder::new(5);
    assert!(order.push(result(4)).is_err());
    order.push(result(6)).unwrap();
    assert!(order.push(result(6)).is_err());
}
