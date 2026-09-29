//! A local language model through mistral.rs (candle, pure Rust) on the GPU.
//!
//! **Role:** load a GGUF model from the models folder once, and answer each request with JSON
//! constrained to the request's schema, thinking switched off.
//!
//! **Position:** a [`LanguageModel`] for the adjudication stage, behind the `mistralrs` feature;
//! mistral.rs is async, so the backend owns a Tokio runtime and blocks on each request.
//!
//! **Signals and state:** the model on the GPU and the runtime; `HF_HUB_OFFLINE` is expected so
//! nothing is fetched from the network.
//!
//! **Invariants:** an answer that is not valid JSON is an error, never an empty success.

#![cfg(feature = "mistralrs")]

use std::path::Path;

use mistralrs::{Constraint, GgufModelBuilder, Model, RequestBuilder, TextMessageRole};

use super::{Completion, LanguageModel, LlmError};

/// An opened local model.
pub struct MistralRs {
    runtime: tokio::runtime::Runtime,
    model: Model,
    name: String,
    /// The longest answer, in tokens.
    pub max_tokens: usize,
}

impl MistralRs {
    /// Load `file` (a GGUF with its tokenizer and chat template) from `dir`.
    pub fn open(dir: &Path, file: &str) -> Result<MistralRs, LlmError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| LlmError(e.to_string()))?;
        let model = runtime
            .block_on(
                GgufModelBuilder::new(dir.to_string_lossy(), vec![file.to_string()])
                    .with_max_num_seqs(1)
                    .build(),
            )
            .map_err(|e| LlmError(format!("loading {file}: {e}")))?;
        Ok(MistralRs {
            runtime,
            model,
            name: format!("mistral.rs/{file}"),
            max_tokens: 12_000,
        })
    }
}

impl LanguageModel for MistralRs {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn complete_json(
        &mut self,
        system: &str,
        user: &str,
        schema: &serde_json::Value,
    ) -> Result<Completion, LlmError> {
        let request = RequestBuilder::new()
            .set_constraint(Constraint::JsonSchema(schema.clone()))
            .set_sampler_max_len(self.max_tokens)
            .set_sampler_temperature(0.0)
            .enable_thinking(false)
            .add_message(TextMessageRole::System, system)
            .add_message(TextMessageRole::User, user);
        let response = self
            .runtime
            .block_on(self.model.send_chat_request(request))
            .map_err(|e| LlmError(e.to_string()))?;
        let text = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .ok_or_else(|| LlmError::invalid_response("the answer is empty"))?;
        let json = serde_json::from_str(&text).map_err(|e| {
            LlmError::invalid_response(format!(
                "the answer is not JSON ({e}): {}",
                text.chars().take(300).collect::<String>()
            ))
        })?;
        Ok(Completion {
            json,
            input_tokens: response.usage.prompt_tokens as u64,
            output_tokens: response.usage.completion_tokens as u64,
            cost_usd: None,
        })
    }
}
