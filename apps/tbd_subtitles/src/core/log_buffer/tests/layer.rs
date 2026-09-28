use tracing::Level;
use tracing_subscriber::layer::SubscriberExt as _;

use super::*;
use crate::core::log_buffer::{KeptCall, LogLine};

/// Run `body` under a subscriber with only a console layer over a fresh buffer; what it kept.
fn capture(body: impl FnOnce()) -> (Vec<LogLine>, Vec<KeptCall>) {
    let buffer = Arc::new(LogBuffer::new());
    let subscriber = tracing_subscriber::registry().with(ConsoleLayer::new(buffer.clone()));
    tracing::subscriber::with_default(subscriber, body);
    (buffer.since(0), buffer.calls_since(0))
}

#[test]
fn a_line_keeps_the_message_then_the_other_fields() {
    let (lines, _) = capture(|| {
        tracing::warn!(target: "ffmpeg", pid = 42, file = "a b.mkv", "exited with {}", 1);
        tracing::info!(stage = "transcribe");
    });
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].level, Level::WARN);
    assert_eq!(lines[0].target, "ffmpeg");
    assert_eq!(lines[0].message, "exited with 1 pid=42 file=a b.mkv");
    assert_eq!(lines[1].message, "stage=transcribe");
}

#[test]
fn a_line_takes_its_video_and_step_from_the_spans_around_it() {
    let (lines, _) = capture(|| {
        let job = tracing::info_span!("job", video = "Dressrosa 12");
        let _job = job.enter();
        tracing::info!("before any step");
        let step = tracing::info_span!("step", step = "adjudicate");
        let _step = step.enter();
        tracing::info!("inside the step");
    });
    assert_eq!(lines[0].video.as_deref(), Some("Dressrosa 12"));
    assert_eq!(lines[0].step, None);
    assert_eq!(lines[1].video.as_deref(), Some("Dressrosa 12"));
    assert_eq!(lines[1].step.as_deref(), Some("adjudicate"));
}

#[test]
fn an_event_s_own_context_and_call_win_and_leave_its_text() {
    let (lines, _) = capture(|| {
        let step = tracing::info_span!("step", step = "cues");
        let _step = step.enter();
        tracing::info!(
            video = "Dressrosa 13",
            step = "shot_scan",
            call = "1-2",
            "finished"
        );
    });
    assert_eq!(lines[0].message, "finished");
    assert_eq!(lines[0].video.as_deref(), Some("Dressrosa 13"));
    assert_eq!(lines[0].step.as_deref(), Some("shot_scan"));
    assert_eq!(lines[0].call.as_deref(), Some("1-2"));
}

#[test]
fn a_model_call_is_kept_as_a_call_under_its_step_and_never_as_a_line() {
    let call = ModelExchange {
        id: "5-1".into(),
        model: "sonnet".into(),
        message: "U0012 hello".into(),
        ..ModelExchange::default()
    };
    let json = serde_json::to_string(&call).unwrap();
    let (lines, calls) = capture(|| {
        let job = tracing::info_span!("job", video = "Dressrosa 12");
        let _job = job.enter();
        tracing::trace!(target: "model_exchange", exchange = %json);
        tracing::trace!(target: "model_exchange", exchange = "{not json");
    });
    assert!(lines.is_empty(), "{lines:?}");
    assert_eq!(calls.len(), 1, "a broken exchange is dropped");
    assert_eq!(calls[0].call, call);
    assert_eq!(calls[0].video.as_deref(), Some("Dressrosa 12"));
}

#[test]
fn a_worker_s_line_is_shown_for_what_it_is() {
    let (lines, _) = capture(|| {
        tracing::debug!(
            target: "child_process",
            "tbd-subtitles[9] 2026-09-28T10:00:00.000001Z  INFO inference::llm::call_log: claude sonnet: done call=9-4"
        );
        tracing::debug!(target: "child_process", "ffmpeg[3] frame=1");
    });
    assert_eq!(lines[0].level, Level::INFO);
    assert_eq!(lines[0].target, "inference::llm::call_log");
    assert_eq!(lines[0].message, "claude sonnet: done");
    assert_eq!(lines[0].call.as_deref(), Some("9-4"));
    assert_eq!(lines[1].target, "child_process");
    assert_eq!(lines[1].message, "ffmpeg[3] frame=1");
}
