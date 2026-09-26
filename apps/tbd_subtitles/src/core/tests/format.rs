use super::*;

#[test]
fn sizes_switch_to_gib_at_one_gib() {
    assert_eq!(size(86_870_208), "82.8 MiB");
    assert_eq!(size(3_095_033_483), "2.9 GiB");
}
