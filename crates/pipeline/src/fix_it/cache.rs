//! Fix It's answered calls kept on disk, so a stopped or killed run picks up where it stopped
//! without asking the model again.
//!
//! **Role:** wrap a model so each call is looked up by the SHA-256 of the model's name, the system
//! prompt, the message and the schema; a kept answer is handed back at no cost, and a new answer
//! is kept before it is handed on.
//!
//! **Position:** used by `fix_it::fix_job` around every model it makes; the folder is
//! `fix/calls/` in the work directory, removed once a run finishes.
//!
//! **Signals and state:** one JSON file per answered call; counts the calls answered from disk.
//!
//! **Invariants:** only a successful answer is kept; a file that cannot be read is a miss, never
//! an error; a kept answer costs no tokens again.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use inference::llm::{Completion, LanguageModel, LlmError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::work_dir;

/// One kept answer.
#[derive(Debug, Serialize, Deserialize)]
struct Kept {
    json: Value,
    input_tokens: u64,
    output_tokens: u64,
    cost_usd: Option<f64>,
}

/// A model whose answers are kept in `dir`.
pub struct CachedModel {
    inner: Box<dyn LanguageModel + Send>,
    dir: PathBuf,
    /// The name the answers are kept under, such as `opus`.
    model: String,
    hits: Arc<AtomicUsize>,
}

impl CachedModel {
    pub fn new(
        inner: Box<dyn LanguageModel + Send>,
        dir: PathBuf,
        model: &str,
        hits: Arc<AtomicUsize>,
    ) -> CachedModel {
        CachedModel {
            inner,
            dir,
            model: model.to_string(),
            hits,
        }
    }

    fn path(&self, system: &str, user: &str, schema: &Value) -> PathBuf {
        let mut hash = Sha256::new();
        for part in [self.model.as_str(), system, user, &schema.to_string()] {
            hash.update(part.as_bytes());
            hash.update([0]);
        }
        let key: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        self.dir.join(format!("{key}.json"))
    }
}

impl LanguageModel for CachedModel {
    fn name(&self) -> String {
        self.inner.name()
    }

    fn complete_json(
        &mut self,
        system: &str,
        user: &str,
        schema: &Value,
    ) -> Result<Completion, LlmError> {
        let path = self.path(system, user, schema);
        if let Ok(kept) = work_dir::read_json::<Kept>(&path) {
            self.hits.fetch_add(1, Ordering::SeqCst);
            return Ok(Completion {
                json: kept.json,
                input_tokens: 0,
                output_tokens: 0,
                cost_usd: Some(0.0),
            });
        }
        let completion = self.inner.complete_json(system, user, schema)?;
        let kept = Kept {
            json: completion.json.clone(),
            input_tokens: completion.input_tokens,
            output_tokens: completion.output_tokens,
            cost_usd: completion.cost_usd,
        };
        // A kept answer only saves a later call; failing to keep it loses nothing now.
        let _ = work_dir::write_json(&path, &kept);
        Ok(completion)
    }
}
