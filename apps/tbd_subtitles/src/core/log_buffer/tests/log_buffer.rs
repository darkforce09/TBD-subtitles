use super::*;

fn messages(lines: &[LogLine]) -> Vec<&str> {
    lines.iter().map(|line| line.message.as_str()).collect()
}

fn line(buffer: &LogBuffer, level: Level, message: &str) {
    buffer.push(Fresh::new(level, "t", message));
}

#[test]
fn lines_come_back_in_order_with_growing_numbers_and_their_context() {
    let buffer = LogBuffer::new();
    buffer.push(Fresh::new(Level::INFO, "a", "one"));
    buffer.push(Fresh::new(Level::WARN, "b", "two").about("Dressrosa 12", Some("cues")));
    let lines = buffer.since(0);
    assert_eq!(messages(&lines), ["one", "two"]);
    assert_eq!((lines[0].seq, lines[1].seq), (0, 1));
    assert_eq!(lines[1].level, Level::WARN);
    assert_eq!(lines[1].target, "b");
    assert_eq!(lines[1].video.as_deref(), Some("Dressrosa 12"));
    assert_eq!(lines[1].step.as_deref(), Some("cues"));
    assert_eq!(lines[0].video, None);
}

#[test]
fn since_leaves_out_the_lines_already_read() {
    let buffer = LogBuffer::new();
    for n in 0..5 {
        line(&buffer, Level::INFO, &n.to_string());
    }
    assert_eq!(messages(&buffer.since(3)), ["3", "4"]);
    assert!(buffer.since(5).is_empty());
}

#[test]
fn a_full_buffer_drops_its_oldest_line() {
    let buffer = LogBuffer::new();
    for n in 0..=CAPACITY {
        line(&buffer, Level::DEBUG, &n.to_string());
    }
    let lines = buffer.since(0);
    assert_eq!(lines.len(), CAPACITY);
    assert_eq!(lines[0].message, "1");
    assert_eq!(lines[CAPACITY - 1].seq, CAPACITY as u64);
}

#[test]
fn numbers_keep_growing_after_a_clear() {
    let buffer = LogBuffer::new();
    line(&buffer, Level::INFO, "before");
    buffer.clear();
    assert!(buffer.since(0).is_empty());
    line(&buffer, Level::INFO, "after");
    let lines = buffer.since(0);
    assert_eq!(messages(&lines), ["after"]);
    assert_eq!(lines[0].seq, 1);
}

#[test]
fn calls_are_kept_apart_up_to_their_own_cap_and_cleared_apart() {
    let buffer = LogBuffer::new();
    line(&buffer, Level::INFO, "a line");
    for n in 0..=CALL_CAPACITY {
        let call = ModelExchange {
            id: n.to_string(),
            ..ModelExchange::default()
        };
        buffer.push_call(call, Some("Dressrosa 12".into()), None);
    }
    let calls = buffer.calls_since(0);
    assert_eq!(calls.len(), CALL_CAPACITY);
    assert_eq!(calls[0].call.id, "1");
    assert_eq!(calls[0].video.as_deref(), Some("Dressrosa 12"));
    assert_eq!(buffer.calls_since(CALL_CAPACITY as u64).len(), 1);
    buffer.clear_calls();
    assert!(buffer.calls_since(0).is_empty());
    assert_eq!(buffer.since(0).len(), 1, "the lines stay");
}
