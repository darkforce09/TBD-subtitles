//! The headless `claude -p` CLI as a backend: a JSON schema in, the structured answer out, no
//! tools enabled.
//!
//! **Role:** run `claude -p` with the system prompt, the JSON Schema and the user message on
//! stdin, and read `structured_output`, token counts and cost from its JSON result.
//!
//! **Position:** a [`LanguageModel`] for the adjudication stage and Fix It; runs the program
//! through `child_process` with a deadline, and with a cancel flag when one is given.
//!
//! **Signals and state:** runs in an empty working folder with only project settings, so the
//! owner's user-level hooks, plugins and MCP servers never reach the prompt. Resolves `claude` on
//! `PATH`, or `$HOME/.local/bin/claude`, since a desktop-launched app often lacks the shell's
//! `PATH`. Logs one summary line per call (model, input lines, time, tokens, cost), never the
//! prompt or the answer.
//!
//! **Invariants:** no tool is enabled (`--tools ""`), no session is saved, and an answer without
//! `structured_output` is an error, never an empty success; once the cancel flag is set, the
//! running `claude` is killed with its process group and the call is an error.

use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use child_process::{Run, RunError};

use super::{Completion, LanguageModel, LlmError};

/// `claude -p` with a fixed model.
pub struct ClaudeCli {
    pub program: String,
    pub model: String,
    pub timeout: Duration,
    /// An empty folder to run in, so no project instructions are picked up.
    pub cwd: PathBuf,
    /// Once set, the running call is killed and fails.
    pub cancel: Option<Arc<AtomicBool>>,
}

impl ClaudeCli {
    pub fn new(model: &str, cwd: PathBuf) -> ClaudeCli {
        ClaudeCli {
            program: resolve_program(),
            model: model.to_string(),
            timeout: Duration::from_secs(600),
            cwd,
            cancel: None,
        }
    }

    /// The same backend, stopped once `flag` is set.
    pub fn with_cancel(mut self, flag: Arc<AtomicBool>) -> ClaudeCli {
        self.cancel = Some(flag);
        self
    }
}

/// The `claude` CLI: on `PATH` if found there, else `$HOME/.local/bin/claude` if that file
/// exists, else the bare name (so a failure to run it still names what was tried).
pub fn resolve_program() -> String {
    if let Ok(path) = child_process::which("claude") {
        return path.to_string_lossy().into_owned();
    }
    if let Some(home) = std::env::var_os("HOME") {
        let candidate = PathBuf::from(home).join(".local/bin/claude");
        if candidate.is_file() {
            return candidate.to_string_lossy().into_owned();
        }
    }
    "claude".to_string()
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
        let started = Instant::now();
        let answer = self.call(system, user, schema);
        log_call(&self.model, user, started.elapsed(), &answer);
        answer
    }
}

impl ClaudeCli {
    fn call(
        &self,
        system: &str,
        user: &str,
        schema: &serde_json::Value,
    ) -> Result<Completion, LlmError> {
        std::fs::create_dir_all(&self.cwd).map_err(|e| LlmError(e.to_string()))?;
        let run = Run::new(&self.program)
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
            .timeout(self.timeout);
        let (code, stdout, stderr) = match &self.cancel {
            Some(flag) => run_cancellable(run.cancel_on(flag.clone()))?,
            None => {
                let out = run.output().map_err(|e| LlmError(e.to_string()))?;
                (out.code, out.stdout, out.stderr)
            }
        };
        if code != 0 {
            return Err(LlmError(format!(
                "claude exited {code}: {}{}",
                stderr.trim(),
                stdout.chars().take(500).collect::<String>()
            )));
        }
        parse(&stdout)
    }
}

/// One line per call: which model, how much went in, how long it took and what it cost; never
/// the prompt or the answer themselves.
fn log_call(model: &str, user: &str, took: Duration, answer: &Result<Completion, LlmError>) {
    let secs = took.as_secs_f64();
    let lines = user.lines().count();
    match answer {
        Ok(done) => tracing::info!(
            "claude {model}: {lines} input lines answered in {secs:.1} s, {} tokens in, {} out{}",
            done.input_tokens,
            done.output_tokens,
            done.cost_usd
                .map(|usd| format!(", ${usd:.4}"))
                .unwrap_or_default()
        ),
        Err(error) => {
            tracing::warn!("claude {model}: {lines} input lines failed after {secs:.1} s: {error}")
        }
    }
}

/// Run `run` under its watchdog, which kills it once the cancel flag is set: the exit code, the
/// whole stdout and the stderr.
fn run_cancellable(run: Run) -> Result<(i32, String, String), LlmError> {
    let mut running = run.spawn().map_err(|e| LlmError(e.to_string()))?;
    let mut stdout = String::new();
    if let Some(mut pipe) = running.take_stdout() {
        // A killed group closes the pipe, so the read ends and `wait` names the cause.
        let _ = pipe.read_to_string(&mut stdout);
    }
    match running.wait() {
        Ok(finished) => Ok((finished.code, stdout, finished.stderr)),
        Err(RunError::Cancelled { .. }) => Err(LlmError("cancelled".into())),
        Err(e) => Err(LlmError(e.to_string())),
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
