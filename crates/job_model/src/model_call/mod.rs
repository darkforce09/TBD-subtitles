//! One language-model call as the log window shows it: what was sent, what came back, and what
//! it cost.
//!
//! **Role:** the record a backend makes of each call, carried from a worker process to the app
//! as one JSON line on the worker's stdout (`model-call <json>`).
//!
//! **Position:** built by `inference::llm::claude_cli`; parsed by `pipeline::workers` and the app's
//! log buffer.
//!
//! **Signals and state:** none; plain data.
//!
//! **Invariants:** never written to a job's work directory or to a log file; the JSON names stay
//! stable between the app and its workers, which are always built together.

use serde::{Deserialize, Serialize};

/// The prefix of a worker's stdout line that carries a [`ModelExchange`].
pub const WORKER_LINE_PREFIX: &str = "model-call ";

/// One call to a language model.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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

impl ModelExchange {
    /// The worker's stdout line that carries this call.
    pub fn worker_line(&self) -> Option<String> {
        serde_json::to_string(self)
            .ok()
            .map(|json| format!("{WORKER_LINE_PREFIX}{json}"))
    }

    /// The call a worker's stdout line carries; `None` for any other line.
    pub fn from_worker_line(line: &str) -> Option<ModelExchange> {
        serde_json::from_str(line.strip_prefix(WORKER_LINE_PREFIX)?).ok()
    }
}

#[cfg(test)]
#[path = "tests/model_call.rs"]
mod tests;
