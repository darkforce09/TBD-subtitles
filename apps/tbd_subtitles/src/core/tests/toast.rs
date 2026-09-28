use super::*;

#[test]
fn toasts_expire_in_their_time_and_the_oldest_leaves_first() {
    let now = Instant::now();
    let mut toasts: Toasts<&str> = Toasts::default();
    let first = toasts.push(ToastKind::Success, "one", now);
    toasts.push_with(
        ToastKind::Info,
        "two",
        Some(("Undo".into(), "undo")),
        Duration::from_secs(6),
        now,
    );
    toasts.push(ToastKind::Working, "three", now);
    toasts.push(ToastKind::Error, "four", now);
    let texts: Vec<&str> = toasts.shown().iter().map(|t| t.text.as_str()).collect();
    assert_eq!(texts, ["two", "three", "four"], "at most three");
    assert!(toasts.take(first).is_none(), "the oldest left");
    assert_eq!(toasts.next_expiry(), Some(now + SHOWN));
    toasts.expire(now + SHOWN);
    let texts: Vec<&str> = toasts.shown().iter().map(|t| t.text.as_str()).collect();
    assert_eq!(texts, ["two"], "the Undo toast shows longer");
    toasts.expire(now + Duration::from_secs(6));
    assert!(toasts.shown().is_empty());
    assert_eq!(toasts.next_expiry(), None);
}

#[test]
fn a_pressed_button_takes_its_toast_away_with_its_action() {
    let now = Instant::now();
    let mut toasts: Toasts<&str> = Toasts::default();
    let id = toasts.push_with(
        ToastKind::Info,
        "Removed a.",
        Some(("Undo".into(), "undo")),
        SHOWN,
        now,
    );
    let toast = toasts.take(id).expect("shown");
    assert_eq!(toast.action, Some(("Undo".to_string(), "undo")));
    assert!(toasts.take(id).is_none());
    assert_ne!(
        toasts.push(ToastKind::Info, "b", now),
        id,
        "ids are not reused"
    );
}

#[test]
fn chosen_toasts_are_dismissed() {
    let now = Instant::now();
    let mut toasts: Toasts<&str> = Toasts::default();
    toasts.push_with(
        ToastKind::Info,
        "a",
        Some(("Undo".into(), "undo")),
        SHOWN,
        now,
    );
    toasts.push(ToastKind::Success, "b", now);
    toasts.dismiss(|toast| toast.action.is_some());
    let texts: Vec<&str> = toasts.shown().iter().map(|t| t.text.as_str()).collect();
    assert_eq!(texts, ["b"]);
}
