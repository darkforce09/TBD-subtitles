use tracing::Level;

use super::*;

fn line(level: Level, target: &str, message: &str) -> LogLine {
    LogLine {
        seq: 0,
        elapsed: Duration::from_millis(83_456),
        level,
        target: target.to_string(),
        message: message.to_string(),
        video: None,
        step: None,
        call: None,
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
fn a_header_names_the_video_and_the_step_in_words() {
    assert_eq!(
        header(Some("Dressrosa 12"), Some("adjudicate")),
        format!(
            "Dressrosa 12 · {} — {}",
            stage_of(StepName::Adjudicate).title,
            step_title(StepName::Adjudicate)
        )
    );
    assert_eq!(
        header(Some("Dressrosa 12"), Some("fix_it")),
        "Dressrosa 12 · Fix It"
    );
    assert_eq!(header(Some("Dressrosa 12"), None), "Dressrosa 12");
    assert_eq!(header(None, Some("mystery")), "mystery");
    assert_eq!(header(None, None), "");
}

#[test]
fn a_copied_line_says_when_how_bad_who_where_and_what() {
    let mut warned = line(Level::WARN, "child_process", "claude[9] exited 1");
    assert_eq!(
        line_text(&warned),
        "01:23.456  WARN   Program  child_process  claude[9] exited 1"
    );
    warned.video = Some("D12".into());
    warned.step = Some("fix_it".into());
    assert!(line_text(&warned).ends_with("child_process  [D12 · Fix It] claude[9] exited 1"));
}

#[test]
fn copy_writes_the_shown_lines_only() {
    let mut activity = Activity::default();
    activity.append(vec![
        line(Level::DEBUG, "a", "quiet"),
        LogLine {
            seq: 1,
            ..line(Level::ERROR, "b", "loud")
        },
    ]);
    activity.set_level(Level::ERROR);
    assert_eq!(copy_text(&activity), "01:23.456  ERROR  App      b  loud\n");
}
