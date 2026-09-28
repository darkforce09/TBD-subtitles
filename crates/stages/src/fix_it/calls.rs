//! Fix It's model calls: several at once, none once the run is stopped, and what they cost.
//!
//! **Role:** send a list of messages under one system prompt and schema, `workers` at a time,
//! each worker with its own model; hand back each answer in order; count calls, tokens and cost;
//! and record why a call failed or its answer could not be read.
//!
//! **Position:** used by the three passes of `fix_it`.
//!
//! **Signals and state:** the usage behind a mutex, shared by the worker threads of every pass.
//!
//! **Invariants:** no call starts once `stop` is set; an answer that failed or was never asked is
//! `None`, never an empty answer.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use inference::llm::LanguageModel;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Makes one model per worker; for the `claude` CLI, each is its own process per call.
pub type Make<'a> = dyn Fn() -> Box<dyn LanguageModel + Send> + Sync + 'a;

/// What the calls of a run cost.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Usage {
    pub calls: usize,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    /// Why a call failed or its answer could not be read, per call.
    pub failed: Vec<String>,
}

/// The calls of one run.
pub struct Calls<'a> {
    make: &'a Make<'a>,
    workers: usize,
    stop: &'a AtomicBool,
    usage: Mutex<Usage>,
}

impl<'a> Calls<'a> {
    pub fn new(make: &'a Make<'a>, workers: usize, stop: &'a AtomicBool) -> Calls<'a> {
        Calls {
            make,
            workers: workers.max(1),
            stop,
            usage: Mutex::new(Usage::default()),
        }
    }

    /// Whether the run is stopped.
    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// Ask each of `messages` with `system` and `schema`; each answer in the order asked. `label`
    /// names the calls in a failure; `progress` hears `(calls done, calls)`.
    pub fn ask_all(
        &self,
        label: &str,
        system: &str,
        schema: &Value,
        messages: &[String],
        progress: &(dyn Fn(usize, usize) + Sync),
    ) -> Vec<Option<Value>> {
        let answers: Vec<Mutex<Option<Value>>> =
            messages.iter().map(|_| Mutex::new(None)).collect();
        let next = AtomicUsize::new(0);
        let done = AtomicUsize::new(0);
        let workers = self.workers.min(messages.len());
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| {
                    let mut model = (self.make)();
                    loop {
                        let i = next.fetch_add(1, Ordering::SeqCst);
                        if i >= messages.len() || self.stopped() {
                            break;
                        }
                        let answer = self.ask(model.as_mut(), system, &messages[i], schema);
                        match answer {
                            Ok(value) => *lock(&answers[i]) = Some(value),
                            Err(why) => self.fail(&format!(
                                "{label} {} of {}: {why}",
                                i + 1,
                                messages.len()
                            )),
                        }
                        progress(done.fetch_add(1, Ordering::SeqCst) + 1, messages.len());
                    }
                });
            }
        });
        answers
            .into_iter()
            .map(|a| a.into_inner().unwrap_or_else(|e| e.into_inner()))
            .collect()
    }

    fn ask(
        &self,
        model: &mut dyn LanguageModel,
        system: &str,
        user: &str,
        schema: &Value,
    ) -> Result<Value, String> {
        let completion = model.complete_json(system, user, schema);
        let mut usage = lock(&self.usage);
        usage.calls += 1;
        let completion = completion.map_err(|e| e.to_string())?;
        usage.input_tokens += completion.input_tokens;
        usage.output_tokens += completion.output_tokens;
        usage.cost_usd += completion.cost_usd.unwrap_or(0.0);
        Ok(completion.json)
    }

    /// Read `answer` as `T`; a failure is recorded under `label`.
    pub fn read<T: DeserializeOwned>(&self, label: &str, answer: Value) -> Option<T> {
        serde_json::from_value(answer)
            .map_err(|e| {
                self.fail(&format!(
                    "{label}: the answer does not match the schema: {e}"
                ))
            })
            .ok()
    }

    /// Why the last call that failed gave nothing usable.
    pub fn last_failure(&self) -> Option<String> {
        lock(&self.usage).failed.last().cloned()
    }

    /// Record why a call gave nothing usable.
    pub fn fail(&self, why: &str) {
        lock(&self.usage).failed.push(why.to_string());
    }

    /// What every call cost.
    pub fn into_usage(self) -> Usage {
        self.usage.into_inner().unwrap_or_else(|e| e.into_inner())
    }
}

/// A mutex's value, even after a worker panicked holding it.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
