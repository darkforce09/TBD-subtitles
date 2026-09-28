use tracing_subscriber::layer::SubscriberExt as _;

use super::*;

fn messages(lines: &[LogLine]) -> Vec<&str> {
    lines.iter().map(|line| line.message.as_str()).collect()
}

#[test]
fn lines_come_back_in_order_with_growing_numbers() {
    let buffer = LogBuffer::new();
    buffer.push(Level::INFO, "a", "one".to_string());
    buffer.push(Level::WARN, "b", "two".to_string());
    let lines = buffer.since(0);
    assert_eq!(messages(&lines), ["one", "two"]);
    assert_eq!(lines[0].seq, 0);
    assert_eq!(lines[1].seq, 1);
    assert_eq!(lines[1].level, Level::WARN);
    assert_eq!(lines[1].target, "b");
}

#[test]
fn since_leaves_out_the_lines_already_read() {
    let buffer = LogBuffer::new();
    for n in 0..5 {
        buffer.push(Level::INFO, "t", n.to_string());
    }
    assert_eq!(messages(&buffer.since(3)), ["3", "4"]);
    assert!(buffer.since(5).is_empty());
}

#[test]
fn a_full_buffer_drops_its_oldest_line() {
    let buffer = LogBuffer::new();
    for n in 0..=CAPACITY {
        buffer.push(Level::DEBUG, "t", n.to_string());
    }
    let lines = buffer.since(0);
    assert_eq!(lines.len(), CAPACITY);
    assert_eq!(lines[0].message, "1");
    assert_eq!(lines[CAPACITY - 1].seq, CAPACITY as u64);
}

#[test]
fn numbers_keep_growing_after_a_clear() {
    let buffer = LogBuffer::new();
    buffer.push(Level::INFO, "t", "before".to_string());
    buffer.clear();
    assert!(buffer.since(0).is_empty());
    buffer.push(Level::INFO, "t", "after".to_string());
    let lines = buffer.since(0);
    assert_eq!(messages(&lines), ["after"]);
    assert_eq!(lines[0].seq, 1);
}

#[test]
fn the_layer_keeps_the_message_then_the_other_fields() {
    let buffer = Arc::new(LogBuffer::new());
    let subscriber = tracing_subscriber::registry().with(ConsoleLayer::new(buffer.clone()));
    tracing::subscriber::with_default(subscriber, || {
        tracing::warn!(target: "ffmpeg", pid = 42, file = "a b.mkv", "exited with {}", 1);
        tracing::info!(step = "transcribe");
    });
    let lines = buffer.since(0);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].level, Level::WARN);
    assert_eq!(lines[0].target, "ffmpeg");
    assert_eq!(lines[0].message, "exited with 1 pid=42 file=a b.mkv");
    assert_eq!(lines[1].message, "step=transcribe");
}
