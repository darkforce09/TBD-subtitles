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

#[test]
fn a_time_left_is_said_loosely() {
    assert_eq!(about(1140.0), "about 19 min");
    assert_eq!(about(90.0), "about 2 min");
    assert_eq!(about(45.0), "under 2 min");
    assert_eq!(about(5.0), "a few seconds");
}

#[test]
fn line_times_read_to_the_tenth_of_a_second() {
    assert_eq!(clock_tenths(993.44), "16:33.4");
    assert_eq!(clock_tenths(9.24), "0:09.2");
    assert_eq!(clock_tenths(59.96), "1:00.0");
    assert_eq!(clock_tenths(3725.0), "1:02:05.0");
    assert_eq!(clock_tenths(-1.0), "0:00.0");
}

#[test]
fn a_videos_length_reads_as_minutes_and_seconds_below_an_hour() {
    assert_eq!(length(1559.4), "25:59");
    assert_eq!(length(59.6), "1:00");
    assert_eq!(length(3723.0), "1:02:03");
    assert_eq!(length(-1.0), "0:00");
}

#[test]
fn places_in_line_and_counts_read_as_words() {
    let places: Vec<String> = [1, 2, 3, 4, 11, 12, 13, 21, 22, 23, 101, 111]
        .into_iter()
        .map(ordinal)
        .collect();
    assert_eq!(
        places,
        [
            "1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "23rd", "101st",
            "111th"
        ]
    );
    assert_eq!(plural(1, "correction"), "1 correction");
    assert_eq!(plural(2, "correction"), "2 corrections");
    assert_eq!(plural(0, "finished step"), "0 finished steps");
}
