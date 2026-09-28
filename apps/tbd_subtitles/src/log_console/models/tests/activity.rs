use std::time::Duration;

use super::*;

fn line(seq: u64, level: Level, target: &str, message: &str) -> LogLine {
    LogLine {
        seq,
        elapsed: Duration::from_millis(seq),
        level,
        target: target.to_string(),
        message: message.to_string(),
        video: None,
        step: None,
        call: None,
    }
}

fn about(mut line: LogLine, video: &str, step: Option<&str>) -> LogLine {
    line.video = Some(video.to_string());
    line.step = step.map(str::to_string);
    line
}

/// Each row as text: `# video/step` for a header, the message for a line.
fn rows(activity: &Activity) -> Vec<String> {
    (0..activity.rows_len())
        .map(|i| match activity.row(i).unwrap() {
            RowView::Header { video, step } => {
                format!("# {}/{}", video.unwrap_or("-"), step.unwrap_or("-"))
            }
            RowView::Line(line) => line.message.clone(),
        })
        .collect()
}

fn sample() -> Activity {
    let mut activity = Activity::default();
    activity.append(vec![
        line(
            0,
            Level::DEBUG,
            "tbd_subtitles::application",
            "action Queue(Start)",
        ),
        about(line(1, Level::INFO, "job", "Job started"), "D12", None),
        about(
            line(2, Level::INFO, "job", "Step started"),
            "D12",
            Some("adjudicate"),
        ),
        about(
            line(3, Level::DEBUG, "child_process", "claude[9] started"),
            "D12",
            Some("adjudicate"),
        ),
        line(
            4,
            Level::DEBUG,
            "tbd_subtitles::application",
            "action ShowLog(true)",
        ),
        about(
            line(
                5,
                Level::WARN,
                "inference::llm::call_log",
                "claude sonnet: failed",
            ),
            "D12",
            Some("adjudicate"),
        ),
        about(
            line(6, Level::ERROR, "job", "Step failed: boom"),
            "D12",
            Some("cues"),
        ),
    ]);
    activity
}

#[test]
fn a_header_starts_each_new_video_and_step_and_app_lines_never_break_a_group() {
    let activity = sample();
    assert_eq!(
        rows(&activity),
        [
            "action Queue(Start)",
            "# D12/-",
            "Job started",
            "# D12/adjudicate",
            "Step started",
            "claude[9] started",
            "action ShowLog(true)",
            "claude sonnet: failed",
            "# D12/cues",
            "Step failed: boom",
        ]
    );
    assert_eq!(activity.shown_len(), 7);
    assert_eq!(activity.len(), 7);
    assert_eq!(activity.problems(), (1, 1));
    assert_eq!(activity.next(), 7);
}

#[test]
fn the_writer_filter_keeps_one_kind_and_its_headers() {
    let mut activity = sample();
    activity.set_who(Some(Who::Ai));
    assert_eq!(
        rows(&activity),
        ["# D12/adjudicate", "claude sonnet: failed"]
    );
    activity.set_who(Some(Who::App));
    assert_eq!(
        rows(&activity),
        ["action Queue(Start)", "action ShowLog(true)"]
    );
    activity.set_who(None);
    assert_eq!(activity.shown_len(), 7);
}

#[test]
fn a_level_shows_itself_and_the_more_severe_and_new_lines_are_filtered_too() {
    let mut activity = sample();
    activity.set_level(Level::WARN);
    assert_eq!(
        rows(&activity),
        [
            "# D12/adjudicate",
            "claude sonnet: failed",
            "# D12/cues",
            "Step failed: boom"
        ]
    );
    activity.append(vec![line(7, Level::INFO, "job", "later")]);
    assert_eq!(activity.shown_len(), 2);
}

#[test]
fn a_search_matches_the_text_the_source_the_video_or_the_step() {
    let mut activity = sample();
    activity.set_search("claude".into());
    assert_eq!(activity.shown_len(), 2);
    activity.set_search("cues".into());
    assert_eq!(rows(&activity), ["# D12/cues", "Step failed: boom"]);
    activity.set_search("call_log".into());
    assert_eq!(activity.shown_len(), 1);
    activity.set_search(String::new());
    assert_eq!(activity.shown_len(), 7);
}

#[test]
fn a_selected_line_stays_open_until_it_is_cleared() {
    let mut activity = sample();
    activity.select(Some(5));
    assert_eq!(
        activity.selected().unwrap().message,
        "claude sonnet: failed"
    );
    activity.set_who(Some(Who::Job));
    assert!(activity.selected().is_some(), "a filter does not close it");
    activity.clear();
    assert!(activity.selected().is_none());
    assert_eq!(activity.len(), 0);
    assert_eq!(activity.problems(), (0, 0));
    assert_eq!(
        activity.next(),
        7,
        "later lines still arrive after the cleared ones"
    );
}

#[test]
fn past_capacity_the_oldest_lines_leave() {
    let mut activity = Activity::default();
    let lines = (0..=CAPACITY as u64)
        .map(|seq| line(seq, Level::INFO, "t", &seq.to_string()))
        .collect();
    activity.append(lines);
    assert_eq!(activity.len(), CAPACITY);
    assert_eq!(activity.shown_len(), CAPACITY);
    assert!(matches!(activity.row(0), Some(RowView::Line(line)) if line.message == "1"));
}
