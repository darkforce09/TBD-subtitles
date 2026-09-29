use super::*;

fn event(start: u64, text: &str) -> Event {
    Event {
        layer: 1,
        start,
        end: start + 1,
        text: text.into(),
    }
}

#[test]
fn repeated_frames_coalesce_without_consuming_the_budget() {
    let mut buffer = EventBuffer {
        limit: 100,
        ..Default::default()
    };
    for frame in 0..10_000 {
        buffer.push(event(frame, "same text")).unwrap();
    }
    assert_eq!(buffer.events.len(), 1);
    assert_eq!(buffer.events[0].end, 10_000);
}

#[test]
fn overflow_preserves_the_existing_events_and_rejects_whole_appends() {
    let mut buffer = EventBuffer {
        limit: 100,
        ..Default::default()
    };
    buffer.push(event(0, "first")).unwrap();
    assert!(buffer.push(event(1, &"x".repeat(100))).is_err());
    let mut other = EventBuffer::default();
    other.push(event(1, &"y".repeat(100))).unwrap();
    assert!(buffer.append(other).is_err());
    assert_eq!(buffer.events.len(), 1);
    assert_eq!(buffer.events[0].text, "first");
}
