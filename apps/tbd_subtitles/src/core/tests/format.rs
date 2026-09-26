use super::*;

#[test]
fn sizes_switch_to_gib_at_one_gib() {
    assert_eq!(size(86_870_208), "82.8 MiB");
    assert_eq!(size(3_095_033_483), "2.9 GiB");
}

#[test]
fn durations_read_in_the_largest_units() {
    assert_eq!(duration(12.4), "12 s");
    assert_eq!(duration(245.0), "4 min 05 s");
    assert_eq!(duration(3725.0), "1 h 02 min");
    assert_eq!(duration(-3.0), "0 s");
}
