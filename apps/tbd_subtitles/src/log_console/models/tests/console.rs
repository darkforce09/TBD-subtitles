use std::time::Duration;

use super::*;

fn line(seq: u64, level: Level, target: &str, message: &str) -> LogLine {
    LogLine {
        seq,
        elapsed: Duration::from_millis(seq),
        level,
        target: target.to_string(),
        message: message.to_string(),
    }
}

fn shown(console: &Console) -> Vec<&str> {
    console.shown().map(|line| line.message.as_str()).collect()
}

fn sample() -> Console {
    let mut console = Console::default();
    console.append(vec![
        line(0, Level::DEBUG, "child_process", "ffmpeg[7] frame=1"),
        line(1, Level::INFO, "job", "Dressrosa 12: asr_parakeet: started"),
        line(2, Level::WARN, "child_process", "claude[9] exited 1"),
        line(3, Level::ERROR, "job", "Dressrosa 12: stopped"),
    ]);
    console
}

#[test]
fn appended_lines_are_all_shown_and_counted() {
    let console = sample();
    assert_eq!(console.len(), 4);
    assert_eq!(console.shown_len(), 4);
    assert_eq!(console.next(), 4);
    assert_eq!(console.problems(), (1, 1));
    assert_eq!(
        console.shown_line(1).unwrap().message,
        "Dressrosa 12: asr_parakeet: started"
    );
    assert!(console.shown_line(4).is_none());
}

#[test]
fn a_level_shows_itself_and_the_more_severe() {
    let mut console = sample();
    console.set_level(Level::WARN);
    assert_eq!(
        shown(&console),
        ["claude[9] exited 1", "Dressrosa 12: stopped"]
    );
    console.set_level(Level::ERROR);
    assert_eq!(shown(&console), ["Dressrosa 12: stopped"]);
    console.append(vec![line(4, Level::INFO, "job", "later")]);
    assert_eq!(
        shown(&console),
        ["Dressrosa 12: stopped"],
        "new lines filter too"
    );
}

#[test]
fn a_search_matches_the_message_or_the_target_ignoring_case() {
    let mut console = sample();
    console.set_search("  CLAUDE ".to_string());
    assert_eq!(shown(&console), ["claude[9] exited 1"]);
    assert_eq!(console.search(), "  CLAUDE ");
    console.set_search("job".to_string());
    assert_eq!(shown(&console).len(), 2);
    console.set_search(String::new());
    assert_eq!(console.shown_len(), 4);
}

#[test]
fn a_clear_forgets_the_lines_but_keeps_reading_after_them() {
    let mut console = sample();
    console.clear();
    assert_eq!(console.len(), 0);
    assert_eq!(console.problems(), (0, 0));
    assert_eq!(console.next(), 4);
    console.append(Vec::new());
    assert_eq!(console.next(), 4, "nothing read moves nothing");
}

#[test]
fn past_capacity_the_oldest_lines_leave() {
    let mut console = Console::default();
    let lines = (0..=CAPACITY as u64)
        .map(|seq| line(seq, Level::INFO, "t", &seq.to_string()))
        .collect();
    console.append(lines);
    assert_eq!(console.len(), CAPACITY);
    assert_eq!(console.shown_len(), CAPACITY);
    assert_eq!(console.shown_line(0).unwrap().message, "1");
}
