use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Instant;

use serde_json::{Value, json};

use super::*;

/// Answers from a script, one per call, noting when each call came.
struct Scripted {
    answers: Arc<Mutex<VecDeque<Result<Value, String>>>>,
    calls: Arc<Mutex<Vec<Instant>>>,
}

impl LanguageModel for Scripted {
    fn name(&self) -> String {
        "scripted".into()
    }

    fn complete_json(&mut self, _: &str, _: &str, _: &Value) -> Result<Completion, LlmError> {
        self.calls.lock().unwrap().push(Instant::now());
        let answer = self
            .answers
            .lock()
            .unwrap()
            .pop_front()
            .expect("no answer left");
        answer
            .map(|json| Completion {
                json,
                input_tokens: 1,
                output_tokens: 1,
                cost_usd: None,
            })
            .map_err(LlmError)
    }
}

type Calls = Arc<Mutex<Vec<Instant>>>;

/// A gated fake over `answers`, with delays of `ms` milliseconds, at a gate of one slot.
fn gated(answers: Vec<Result<Value, String>>, ms: &[u64]) -> (Gated<Scripted>, Calls) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let inner = Scripted {
        answers: Arc::new(Mutex::new(answers.into())),
        calls: calls.clone(),
    };
    let retry = Retry {
        delays: ms.iter().copied().map(Duration::from_millis).collect(),
    };
    let model = Gated::new(
        inner,
        CallGate::new(1).seat(),
        Arc::new(AtomicBool::new(false)),
    )
    .with_retry(retry);
    (model, calls)
}

fn busy() -> Result<Value, String> {
    Err("claude exited 1: API Error: 529 Overloaded".into())
}

fn ask(model: &mut Gated<Scripted>) -> Result<Completion, LlmError> {
    model.complete_json("system", "user", &json!({}))
}

#[test]
fn a_busy_answer_is_asked_again_after_each_delay() {
    let (mut model, calls) = gated(vec![busy(), busy(), Ok(json!({"ok": 1}))], &[20, 40, 80]);
    let answer = ask(&mut model).unwrap();
    assert_eq!(answer.json, json!({"ok": 1}));
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 3);
    assert!(calls[1] - calls[0] >= Duration::from_millis(20));
    assert!(calls[2] - calls[1] >= Duration::from_millis(40));
}

#[test]
fn a_lasting_failure_is_not_asked_again() {
    let (mut model, calls) = gated(vec![Err("not logged in".into())], &[1, 1]);
    let error = ask(&mut model).unwrap_err();
    assert_eq!(error.0, "not logged in");
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[test]
fn the_last_error_comes_back_once_retries_run_out() {
    let (mut model, calls) = gated(vec![busy(), busy(), busy()], &[1, 1]);
    let error = ask(&mut model).unwrap_err();
    assert_eq!(
        error.0,
        "claude exited 1: API Error: 529 Overloaded (after 3 tries)"
    );
    assert_eq!(calls.lock().unwrap().len(), 3);
}

#[test]
fn stop_during_a_pause_ends_the_call() {
    let (mut model, calls) = gated(vec![busy(), Ok(json!({}))], &[60_000]);
    let cancel = model.cancel.clone();
    let stopper = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        cancel.store(true, Ordering::SeqCst);
    });
    let started = Instant::now();
    let error = ask(&mut model).unwrap_err();
    stopper.join().unwrap();
    assert_eq!(error.0, "cancelled");
    assert!(started.elapsed() < Duration::from_millis(500));
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[test]
fn the_slot_is_free_during_a_pause() {
    let (mut model, _calls) = gated(vec![busy(), Ok(json!({}))], &[300]);
    let gate = model.gate().clone();
    let other = gate.seat();
    let asking = std::thread::spawn(move || ask(&mut model).is_ok());
    let started = Instant::now();
    // The first call fails at once; the model is then in its 300 ms pause.
    std::thread::sleep(Duration::from_millis(30));
    let permit = other.acquire(&AtomicBool::new(false));
    assert!(permit.is_some());
    assert!(
        started.elapsed() < Duration::from_millis(250),
        "the pause held the slot"
    );
    drop(permit);
    assert!(asking.join().unwrap());
}

#[test]
fn transient_errors_are_told_apart() {
    let transient = [
        "claude exited 1: API Error: 529 {\"type\":\"overloaded_error\"}",
        "API Error: 429 Too Many Requests",
        "claude reported an error: \"Rate limit reached\"",
        "rate_limit_error",
        "Server is Overloaded",
        "status 429",
    ];
    for text in transient {
        assert!(is_transient(&LlmError(text.into())), "{text}");
    }
    let lasting = [
        "claude reported an error: \"Claude AI usage limit reached|1760000000\"",
        "claude exited 1: {\"duration_ms\":4290}",
        "claude exited 1: 15290 tokens",
        "not logged in",
        "cancelled",
        "the result holds no structured_output",
    ];
    for text in lasting {
        assert!(!is_transient(&LlmError(text.into())), "{text}");
    }
}
