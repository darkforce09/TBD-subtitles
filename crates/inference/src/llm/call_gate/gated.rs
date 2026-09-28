//! A backend whose every call takes a slot at a [`CallGate`], and is asked again after a pause
//! when the provider answers that it is busy.
//!
//! **Role:** wrap a [`LanguageModel`]: each `complete_json` waits for a slot on its seat, calls
//! the inner backend, and frees the slot; a transient failure (rate limit, overloaded, 429, 529)
//! is asked again after each delay of its [`Retry`].
//!
//! **Position:** made by the Fix It runner around each `claude` backend, inside the cache, so an
//! answer kept on disk never takes a slot.
//!
//! **Signals and state:** the seat, the run's cancel flag and the retry delays; logs each pause
//! as a `tracing` warning.
//!
//! **Invariants:** no slot is held during a pause; a set cancel flag ends a wait or a pause with
//! the `cancelled` error; a failure that is not transient comes back at once, never asked again.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::{CallGate, CallSeat, SLICE};
use crate::llm::{Completion, LanguageModel, LlmError};

/// The pauses before asking again after a transient failure, one per retry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retry {
    pub delays: Vec<Duration>,
}

impl Default for Retry {
    /// Three retries: after 30 s, 60 s and 120 s.
    fn default() -> Retry {
        Retry {
            delays: [30, 60, 120].map(Duration::from_secs).to_vec(),
        }
    }
}

/// `inner` with every call through a seat at a shared [`CallGate`].
pub struct Gated<M> {
    inner: M,
    seat: CallSeat,
    cancel: Arc<AtomicBool>,
    retry: Retry,
}

impl<M: LanguageModel> Gated<M> {
    /// `inner` taking its slots on `seat`, stopped once `cancel` is set.
    pub fn new(inner: M, seat: CallSeat, cancel: Arc<AtomicBool>) -> Gated<M> {
        Gated {
            inner,
            seat,
            cancel,
            retry: Retry::default(),
        }
    }

    /// The same backend with other retry delays.
    pub fn with_retry(mut self, retry: Retry) -> Gated<M> {
        self.retry = retry;
        self
    }

    /// The gate the calls pass.
    pub fn gate(&self) -> &Arc<CallGate> {
        self.seat.gate()
    }

    /// Wait `delay` without a slot; false once the cancel flag is set.
    fn pause(&self, delay: Duration) -> bool {
        let started = Instant::now();
        loop {
            if self.cancel.load(Ordering::SeqCst) {
                return false;
            }
            let Some(left) = delay
                .checked_sub(started.elapsed())
                .filter(|d| !d.is_zero())
            else {
                return true;
            };
            std::thread::sleep(left.min(SLICE));
        }
    }
}

fn cancelled() -> LlmError {
    LlmError("cancelled".into())
}

impl<M: LanguageModel> LanguageModel for Gated<M> {
    fn name(&self) -> String {
        self.inner.name()
    }

    fn complete_json(
        &mut self,
        system: &str,
        user: &str,
        schema: &serde_json::Value,
    ) -> Result<Completion, LlmError> {
        let mut tries = 0;
        loop {
            let Some(permit) = self.seat.acquire(&self.cancel) else {
                return Err(cancelled());
            };
            let answer = self.inner.complete_json(system, user, schema);
            drop(permit);
            tries += 1;
            let error = match answer {
                Ok(completion) => return Ok(completion),
                Err(error) if !is_transient(&error) => return Err(error),
                Err(error) => error,
            };
            let Some(&delay) = self.retry.delays.get(tries - 1) else {
                if tries == 1 {
                    return Err(error);
                }
                return Err(LlmError(format!("{} (after {tries} tries)", error.0)));
            };
            tracing::warn!(
                "claude is busy ({}); asking again in {} s",
                error.0,
                delay.as_secs_f64()
            );
            if !self.pause(delay) {
                return Err(cancelled());
            }
        }
    }
}

/// Whether `error` says the provider is busy for now (rate limited or overloaded), so the same
/// call may succeed later. A used-up subscription ("usage limit") is not transient.
pub fn is_transient(error: &LlmError) -> bool {
    let text = error.0.to_lowercase();
    let busy = [
        "rate limit",
        "rate_limit",
        "overloaded",
        "too many requests",
    ];
    if busy.iter().any(|phrase| text.contains(phrase)) {
        return true;
    }
    text.split(|c: char| !c.is_ascii_digit())
        .any(|number| number == "429" || number == "529")
}

#[cfg(test)]
#[path = "tests/gated.rs"]
mod tests;
