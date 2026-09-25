//! The headless `claude -p` CLI as a backend: a JSON schema in, the structured answer out, no
//! tools enabled.
//!
//! **Role:** run `claude -p` with the system prompt, the JSON Schema and the user message on
//! stdin, and read `structured_output`, token counts and cost from its JSON result.
//!
//! **Position:** a [`LanguageModel`] for the adjudication stage; runs the program through
//! `child_process` with a deadline.
//!
//! **Signals and state:** runs in an empty working folder with only project settings, so the
//! owner's user-level hooks, plugins and MCP servers never reach the prompt.
//!
//! **Invariants:** no tool is enabled (`--tools ""`), no session is saved, and an answer without
//! `structured_output` is an error, never an empty success.

use std::path::PathBuf;
use std::time::Duration;

use child_process::Run;

use super::{Completion, LanguageModel, LlmError};

/// `claude -p` with a fixed model.
pub struct ClaudeCli {
    pub program: String,
    pub model: String,
    pub timeout: Duration,
    /// An empty folder to run in, so no project instructions are picked up.
    pub cwd: PathBuf,
}

impl ClaudeCli {
    pub fn new(model: &str, cwd: PathBuf) -> ClaudeCli {
        ClaudeCli {
            program: "claude".to_string(),
            model: model.to_string(),
            timeout: Duration::from_secs(600),
            cwd,
        }
    }
}

impl LanguageModel for ClaudeCli {
    fn name(&self) -> String {
        format!("claude-cli/{}", self.model)
    }

    fn complete_json(
        &mut self,
        system: &str,
        user: &str,
        schema: &serde_json::Value,
    ) -> Result<Completion, LlmError> {
        std::fs::create_dir_all(&self.cwd).map_err(|e| LlmError(e.to_string()))?;
        let out = Run::new(&self.program)
            .args(["-p", "--output-format", "json", "--json-schema"])
            .arg(schema.to_string())
            .args([
                "--tools",
                "",
                "--no-session-persistence",
                "--strict-mcp-config",
                "--disable-slash-commands",
                "--setting-sources",
                "project",
                "--system-prompt",
            ])
            .arg(system)
            .arg("--model")
            .arg(&self.model)
            .cwd(&self.cwd)
            .stdin(user)
            .timeout(self.timeout)
            .output()
            .map_err(|e| LlmError(e.to_string()))?;
        if out.code != 0 {
            return Err(LlmError(format!(
                "claude exited {}: {}{}",
                out.code,
                out.stderr.trim(),
                out.stdout.chars().take(500).collect::<String>()
            )));
        }
        parse(&out.stdout)
    }
}

/// Read the CLI's JSON result.
pub fn parse(stdout: &str) -> Result<Completion, LlmError> {
    let value: serde_json::Value =
        serde_json::from_str(stdout).map_err(|e| LlmError(format!("unreadable result: {e}")))?;
    if value.get("is_error").and_then(|v| v.as_bool()) == Some(true) {
        return Err(LlmError(format!(
            "claude reported an error: {}",
            value.get("result").cloned().unwrap_or_default()
        )));
    }
    let json = value
        .get("structured_output")
        .cloned()
        .filter(|v| !v.is_null())
        .ok_or_else(|| LlmError("the result holds no structured_output".into()))?;
    let usage = value.get("usage");
    let count = |key: &str| {
        usage
            .and_then(|u| u.get(key))
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
    };
    Ok(Completion {
        json,
        input_tokens: count("input_tokens")
            + count("cache_creation_input_tokens")
            + count("cache_read_input_tokens"),
        output_tokens: count("output_tokens"),
        cost_usd: value.get("total_cost_usd").and_then(|v| v.as_f64()),
    })
}

#[cfg(test)]
#[path = "tests/claude_cli.rs"]
mod tests;
