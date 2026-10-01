//! One language-model call as the log window shows it: what was sent, what came back, and what
//! it cost.
//!
//! **Role:** the record a backend makes of each call, carried from a worker process to the app
//! as its JSON in one `ModelCall` frame of the worker channel.
//!
//! **Position:** built by `inference::llm::call_log`; parsed by `pipeline::workers` and the app's
//! log buffer.
//!
//! **Signals and state:** none; plain data.
//!
//! **Invariants:** never written to a job's work directory or to a log file; the JSON names stay
//! stable between the app and its workers, which are always built together.

use serde::{Deserialize, Serialize};

/// One call to a language model.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct ModelExchange {
    /// Unique in a run of the app: `<pid>-<n>`, the process and its count of calls.
    pub id: String,
    /// The model as the backend names it, such as `sonnet`.
    pub model: String,
    /// Why the call was made, such as "words of the batch from U0012"; empty when unsaid.
    pub purpose: String,
    /// The system prompt.
    pub system: String,
    /// The user message.
    pub message: String,
    /// The JSON Schema the answer must match, pretty-printed.
    pub schema: String,
    /// The structured answer pretty-printed, or what the backend printed when it had none.
    pub answer: String,
    /// Why the call gave no usable answer; `None` when it did.
    pub error: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: Option<f64>,
    /// The call's wall time.
    pub seconds: f64,
}

#[cfg(test)]
#[path = "tests/model_call.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/archive.rs"]
mod archive_tests;
