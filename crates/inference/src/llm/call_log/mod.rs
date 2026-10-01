//! Each language-model call, logged: why it was made, a one-line summary, and the whole exchange
//! for the app's log window.
//!
//! **Role:** hold the purpose of the calls a thread makes ([`purpose`]), number each call, and
//! emit its summary (`info`) and its [`ModelExchange`] (`trace`, target [`EXCHANGE_TARGET`]).
//!
//! **Position:** called by each backend around every call; the purposes are set by the stages
//! that ask (adjudication, sound cues, Fix It, on-screen text). The app's subscriber decides
//! where the events go: the summary to stderr, the log file and the log window; the exchange to
//! the log window only, as a `ModelCall` frame on the worker channel when the call ran in a
//! worker.
//!
//! **Signals and state:** a per-thread stack of purposes; a process-wide count of calls.
//!
//! **Invariants:** the summary never holds the prompt or the answer; the exchange is built only
//! when a subscriber wants it; a purpose lasts as long as its guard, and an inner one hides the
//! outer until it ends.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use job_model::model_call::ModelExchange;

use super::{Completion, LlmError};

/// The `tracing` target of the event that carries a whole exchange.
pub const EXCHANGE_TARGET: &str = "model_exchange";

thread_local! {
    static PURPOSES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

static CALLS: AtomicU64 = AtomicU64::new(0);

/// While the returned guard lives, the calls this thread makes are "for" `text`.
pub fn purpose(text: impl Into<String>) -> PurposeGuard {
    PURPOSES.with(|stack| stack.borrow_mut().push(text.into()));
    PurposeGuard {
        _not_send: std::marker::PhantomData,
    }
}

/// The purpose of this thread's calls now; empty when none is set.
pub fn current_purpose() -> String {
    PURPOSES.with(|stack| stack.borrow().last().cloned().unwrap_or_default())
}

/// Ends its purpose when dropped; stays on the thread that made it.
pub struct PurposeGuard {
    _not_send: std::marker::PhantomData<*const ()>,
}

impl Drop for PurposeGuard {
    fn drop(&mut self) {
        PURPOSES.with(|stack| stack.borrow_mut().pop());
    }
}

/// What a backend sent in one call.
pub struct Sent<'a> {
    pub model: &'a str,
    pub system: &'a str,
    pub message: &'a str,
    pub schema: &'a serde_json::Value,
}

/// Log a finished call: the exchange, when a subscriber wants it, then the summary. `printed` is
/// what the backend printed, kept as the answer when there is no structured one.
pub fn log_call(
    sent: &Sent<'_>,
    took: Duration,
    answer: &Result<Completion, LlmError>,
    printed: &str,
) {
    let id = format!(
        "{}-{}",
        std::process::id(),
        CALLS.fetch_add(1, Ordering::Relaxed) + 1
    );
    let purpose = current_purpose();
    if tracing::enabled!(target: EXCHANGE_TARGET, tracing::Level::TRACE) {
        let exchange = exchange(&id, &purpose, sent, took, answer, printed);
        if let Ok(json) = serde_json::to_string(&exchange) {
            tracing::trace!(target: EXCHANGE_TARGET, exchange = %json);
        }
    }
    let secs = took.as_secs_f64();
    let lines = sent.message.lines().count();
    let model = sent.model;
    let about = if purpose.is_empty() {
        String::new()
    } else {
        format!(" · {purpose}")
    };
    match answer {
        Ok(done) => tracing::info!(
            call = %id,
            "model {model}{about}: {lines} lines answered in {secs:.1} s, {} tokens in, {} out{}",
            done.input_tokens,
            done.output_tokens,
            done.cost_usd.map(|usd| format!(", ${usd:.4}")).unwrap_or_default()
        ),
        Err(error) => tracing::warn!(
            call = %id,
            "model {model}{about}: {lines} lines failed after {secs:.1} s: {error}"
        ),
    }
}

/// The whole call as the log window shows it.
pub fn exchange(
    id: &str,
    purpose: &str,
    sent: &Sent<'_>,
    took: Duration,
    answer: &Result<Completion, LlmError>,
    printed: &str,
) -> ModelExchange {
    let pretty = |value: &serde_json::Value| {
        serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
    };
    let (answer_text, error, input_tokens, output_tokens, cost_usd) = match answer {
        Ok(done) => (
            pretty(&done.json),
            None,
            done.input_tokens,
            done.output_tokens,
            done.cost_usd,
        ),
        Err(error) => (printed.to_string(), Some(error.to_string()), 0, 0, None),
    };
    ModelExchange {
        id: id.to_string(),
        model: sent.model.to_string(),
        purpose: purpose.to_string(),
        system: sent.system.to_string(),
        message: without_image_data(sent.message),
        schema: pretty(sent.schema),
        answer: answer_text,
        error,
        input_tokens,
        output_tokens,
        cost_usd,
        seconds: took.as_secs_f64(),
    }
}

/// The message with every base64 image payload of a stream-json user line replaced by its size,
/// so the log window never holds megabytes of pixels; other text passes through unchanged.
pub fn without_image_data(message: &str) -> String {
    let elide = |line: &str| -> Option<String> {
        let mut value: serde_json::Value = serde_json::from_str(line).ok()?;
        let content = value
            .get_mut("message")?
            .get_mut("content")?
            .as_array_mut()?;
        let mut elided = false;
        for block in content.iter_mut() {
            if block.get("type").and_then(|kind| kind.as_str()) != Some("image") {
                continue;
            }
            let Some(data) = block
                .get_mut("source")
                .and_then(|source| source.get_mut("data"))
            else {
                continue;
            };
            if let Some(text) = data.as_str() {
                *data = serde_json::Value::String(format!("[{} base64 bytes]", text.len()));
                elided = true;
            }
        }
        elided.then(|| value.to_string())
    };
    if !message.contains("\"image\"") {
        return message.to_string();
    }
    message
        .lines()
        .map(|line| elide(line).unwrap_or_else(|| line.to_string()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
#[path = "tests/call_log.rs"]
mod tests;
