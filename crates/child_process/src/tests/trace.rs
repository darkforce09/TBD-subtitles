use std::sync::{Arc, Mutex, OnceLock};

use tracing::Level;
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt as _};

use super::*;
use crate::Run;

type Events = Arc<Mutex<Vec<(Level, String)>>>;

/// Every event logged in this test binary; drain threads log from threads of their own, so the
/// subscriber is the global one.
fn events() -> Events {
    static EVENTS: OnceLock<Events> = OnceLock::new();
    EVENTS
        .get_or_init(|| {
            let events = Events::default();
            let subscriber = tracing_subscriber::registry().with(Capture(events.clone()));
            tracing::subscriber::set_global_default(subscriber).expect("one global subscriber");
            events
        })
        .clone()
}

struct Capture(Events);

impl<S: tracing::Subscriber> Layer<S> for Capture {
    fn on_event(&self, event: &tracing::Event<'_>, _context: Context<'_, S>) {
        struct Message(String);
        impl Visit for Message {
            fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.0 = format!("{value:?}");
                }
            }
        }
        let mut message = Message(String::new());
        event.record(&mut message);
        let level = *event.metadata().level();
        self.0.lock().unwrap().push((level, message.0));
    }
}

/// The events of the child whose command line holds `marker`, after its start.
fn child_events(marker: &str) -> Vec<(Level, String)> {
    let all = events().lock().unwrap().clone();
    let start = all
        .iter()
        .find(|(_, text)| text.contains(" started: ") && text.contains(marker))
        .map(|(_, text)| text.split(' ').next().unwrap().to_string())
        .expect("the child's start is logged");
    all.into_iter()
        .filter(|(_, text)| text.starts_with(&format!("{start} ")))
        .collect()
}

#[test]
fn a_captured_child_logs_its_start_its_stderr_lines_and_its_exit() {
    events();
    let output = Run::new("sh")
        .arg("-c")
        .arg("echo one >&2; echo two >&2; echo out; exit 3 # capture-marker")
        .output()
        .unwrap();
    assert_eq!(
        output.stderr, "one\ntwo\n",
        "the text handed back is unchanged"
    );
    let logged = child_events("capture-marker");
    let texts: Vec<&str> = logged.iter().map(|(_, text)| text.as_str()).collect();
    assert!(texts[0].starts_with("sh["), "{texts:?}");
    assert!(texts[1].ends_with("] one"), "{texts:?}");
    assert!(texts[2].ends_with("] two"), "{texts:?}");
    assert!(texts[3].contains("exited 3 after"), "{texts:?}");
    assert_eq!(logged[3].0, Level::WARN);
    assert!(
        !texts.iter().any(|t| t.ends_with(" out")),
        "stdout is not logged"
    );
}

#[test]
fn a_streamed_child_logs_its_stderr_and_a_clean_exit_as_debug() {
    events();
    let running = Run::new("sh")
        .arg("-c")
        .arg("echo said >&2 # stream-marker")
        .spawn()
        .unwrap();
    assert_eq!(running.wait().unwrap().stderr, "said\n");
    let logged = child_events("stream-marker");
    assert!(logged[1].1.ends_with("] said"), "{logged:?}");
    assert!(logged[2].1.contains("exited 0 after"), "{logged:?}");
    assert_eq!(logged[2].0, Level::DEBUG);
}

#[test]
fn carriage_returns_split_a_line_and_blank_parts_are_skipped() {
    events();
    let tag = Tag::started(
        &Run::new("/usr/bin/progress-printer").arg("split-marker"),
        7,
    );
    tag.line("frame=1\rframe=2\r\n");
    tag.line("   \n");
    let logged = child_events("split-marker");
    let texts: Vec<&str> = logged.iter().map(|(_, text)| text.as_str()).collect();
    assert_eq!(
        texts[1..],
        ["progress-printer[7] frame=1", "progress-printer[7] frame=2"]
    );
}

#[test]
fn long_and_multi_line_arguments_are_logged_as_their_size() {
    let run = Run::new("claude")
        .arg("-p")
        .arg("a b")
        .arg("")
        .arg("x".repeat(LONG_ARGUMENT + 1))
        .arg("l1\nl2");
    assert_eq!(
        command_line(&run),
        format!("claude -p 'a b' '' <{} bytes> <5 bytes>", LONG_ARGUMENT + 1)
    );
}
