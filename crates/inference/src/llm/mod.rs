//! The language-model backends behind one trait: a system prompt, a user message and a JSON
//! Schema in; a JSON value that satisfies the schema out.

pub mod call_gate;
pub mod call_log;
pub mod claude_cli;
pub mod mistral_rs;

pub use call_log::purpose;

use std::fmt;

/// A structured answer and what it cost.
#[derive(Debug, Clone)]
pub struct Completion {
    pub json: serde_json::Value,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// The provider's cost figure, when it reports one.
    pub cost_usd: Option<f64>,
}

/// Why a language model gave no usable answer.
#[derive(Debug)]
pub struct LlmError(pub String);

impl LlmError {
    /// A model answered, but its content cannot satisfy the caller's structured contract.
    pub fn invalid_response(message: impl fmt::Display) -> Self {
        Self(format!("invalid model response: {message}"))
    }

    /// Distinguish per-occurrence uncertainty from runtime, transport or cancellation failure.
    pub fn is_invalid_response(&self) -> bool {
        self.0.starts_with("invalid model response: ")
    }
}

impl fmt::Display for LlmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LlmError {}

/// A backend that answers in JSON matching a schema.
pub trait LanguageModel {
    /// The backend and model, as recorded in reports.
    fn name(&self) -> String;
    fn complete_json(
        &mut self,
        system: &str,
        user: &str,
        schema: &serde_json::Value,
    ) -> Result<Completion, LlmError>;
}
