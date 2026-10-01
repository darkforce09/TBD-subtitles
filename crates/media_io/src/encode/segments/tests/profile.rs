use super::*;

#[test]
fn ffprobe_profile_names_map_to_both_encoders() {
    let cases = [
        ("Constrained Baseline", "baseline", Some("baseline")),
        ("Baseline", "baseline", Some("baseline")),
        ("Main", "main", Some("main")),
        ("High", "high", Some("high")),
        ("High 10", "high10", None),
    ];
    for (name, x264, nvenc) in cases {
        let profile = H264Profile::from_ffprobe(name).unwrap();
        assert_eq!(profile.x264_name(), x264);
        assert_eq!(profile.nvenc_name(), nvenc);
        assert_eq!(profile.is_ten_bit(), name == "High 10");
    }
    for unmatched in ["High 4:2:2", "High 4:4:4 Predictive", "Extended", ""] {
        assert_eq!(H264Profile::from_ffprobe(unmatched), None, "{unmatched}");
    }
}

#[test]
fn levels_are_named_as_both_encoders_take_them() {
    assert_eq!(H264Level(40).name(), "4.0");
    assert_eq!(H264Level(31).name(), "3.1");
    assert_eq!(H264Level(9).name(), "1b");
    assert!(H264Level(41).is_known());
    assert!(!H264Level(0).is_known());
    assert!(!H264Level(43).is_known());
}

#[test]
fn the_peak_rate_is_a_share_of_the_source_rate_within_the_level() {
    // A Main 4.0 source at 3.6 Mb/s.
    let peak = peak_rate(Some(3_600_000), H264Profile::Main, H264Level(40)).unwrap();
    assert_eq!(peak.max_bits_per_s, 5_400_000);
    assert_eq!(peak.buffer_bits, 10_800_000);
    // A fast source is held to Main 4.0's 20 Mb/s and 25 Mb buffer.
    let capped = peak_rate(Some(19_000_000), H264Profile::Main, H264Level(40)).unwrap();
    assert_eq!(capped.max_bits_per_s, 20_000_000);
    assert_eq!(capped.buffer_bits, 25_000_000);
    // High scales the limits by 1.25 and High 10 by 3.
    let high = peak_rate(None, H264Profile::High, H264Level(41)).unwrap();
    assert_eq!(high.max_bits_per_s, 62_500_000);
    assert_eq!(high.buffer_bits, 78_125_000);
    let ten_bit = peak_rate(None, H264Profile::High10, H264Level(30)).unwrap();
    assert_eq!(ten_bit.max_bits_per_s, 30_000_000);
    assert_eq!(peak_rate(Some(1), H264Profile::Main, H264Level(43)), None);
}
