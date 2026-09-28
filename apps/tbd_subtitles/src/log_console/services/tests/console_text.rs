use super::*;

fn line(level: Level, target: &str, message: &str) -> LogLine {
    LogLine {
        seq: 0,
        elapsed: Duration::from_millis(83_456),
        level,
        target: target.to_string(),
        message: message.to_string(),
    }
}

#[test]
fn the_time_counts_minutes_then_hours_in_one_width() {
    assert_eq!(time(Duration::from_millis(83_456)), "01:23.456");
    assert_eq!(time(Duration::ZERO), "00:00.000");
    assert_eq!(time(Duration::from_millis(3_723_456)), "1:02:03.4");
    assert_eq!(
        time(Duration::from_secs(36_000)).len(),
        10,
        "ten hours grows by one"
    );
}

#[test]
fn the_source_is_the_module_s_last_name_in_a_fixed_width() {
    assert_eq!(source("pipeline::workers"), "workers       ");
    assert_eq!(source("job"), "job           ");
    assert_eq!(
        source("tbd_subtitles::core::a_very_long_module_name"),
        "a_very_long_mo"
    );
}

#[test]
fn a_line_is_its_columns_then_its_message() {
    assert_eq!(
        line_text(&line(Level::WARN, "child_process", "claude[9] exited 1")),
        "01:23.456  WARN   child_process   claude[9] exited 1"
    );
}

#[test]
fn copy_writes_the_shown_lines_only() {
    let mut console = Console::default();
    console.append(vec![
        line(Level::DEBUG, "a", "quiet"),
        LogLine {
            seq: 1,
            ..line(Level::ERROR, "b", "loud")
        },
    ]);
    console.set_level(Level::ERROR);
    assert_eq!(
        copy_text(&console),
        "01:23.456  ERROR  b               loud\n"
    );
}
