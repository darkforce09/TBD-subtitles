//! The headless `claude -p` CLI as a backend: a JSON schema in, the structured answer out, no
//! tools enabled.
//!
//! **Role:** run `claude -p` with the system prompt, the JSON Schema and the user message on
//! stdin (plain text, or one stream-json message holding the text and then each PNG image), and
//! read `structured_output`, token counts and cost from its JSON result.
//!
//! **Position:** a [`LanguageModel`] for the adjudication stage and Fix It, and the image backend
//! of the visual translation stage (a keyframe and its crops per call); runs the program through
//! `child_process` with a deadline, and with a cancel flag when one is given.
//!
//! **Signals and state:** runs in an empty working folder with only project settings, so the
//! owner's user-level hooks, plugins and MCP servers never reach the prompt. Resolves `claude` on
//! `PATH`, or `$HOME/.local/bin/claude`, since a desktop-launched app often lacks the shell's
//! `PATH`. Logs each call through `call_log`: a summary line, and the whole exchange for the
//! app's log window.
//!
//! **Invariants:** no tool is enabled (`--tools ""`), no session is saved, and an answer without
//! `structured_output` is an error, never an empty success; once the cancel flag is set, the
//! running `claude` is killed with its process group and the call is an error.

mod shared_slots;

pub use shared_slots::set_shared_limit;

use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use child_process::{Run, RunError};

use super::call_log::{self, Sent};
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

    /// Ask about several PNG images (base64) in one user message, text first, images in order,
    /// with the same isolation and schema as text. Without images it is the plain text call of
    /// `complete_json`.
    pub fn complete_images_json(
        &mut self,
        system: &str,
        user: &str,
        schema: &serde_json::Value,
        pngs: &[String],
    ) -> Result<Completion, LlmError> {
        let (input, stream) = request_input(user, pngs);
        self.complete_request(system, &input, schema, stream)
    }

    /// One image: the same as `complete_images_json` with a single element.
    pub fn complete_image_json(
        &mut self,
        system: &str,
        user: &str,
        schema: &serde_json::Value,
        png_base64: &str,
    ) -> Result<Completion, LlmError> {
        self.complete_images_json(system, user, schema, &[png_base64.to_owned()])
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
        self.complete_request(system, user, schema, false)
    }
}

impl ClaudeCli {
    fn complete_request(
        &self,
        system: &str,
        input: &str,
        schema: &serde_json::Value,
        stream: bool,
    ) -> Result<Completion, LlmError> {
        let started = Instant::now();
        let (answer, printed) = self.call(system, input, schema, stream);
        let sent = Sent {
            model: &self.model,
            system,
            message: input,
            schema,
        };
        call_log::log_call(&sent, started.elapsed(), &answer, &printed);
        answer
    }

    /// The answer, and what `claude` printed on stdout.
    fn call(
        &self,
        system: &str,
        input: &str,
        schema: &serde_json::Value,
        stream: bool,
    ) -> (Result<Completion, LlmError>, String) {
        let _permit = match shared_slots::acquire(self.cancel.as_deref()) {
            Ok(permit) => permit,
            Err(error) => return (Err(error), String::new()),
        };
        if let Err(error) = std::fs::create_dir_all(&self.cwd) {
            return (Err(LlmError(error.to_string())), String::new());
        }
        let run = Run::new(&self.program)
            .args(call_args(&self.model, system, schema, stream))
            .cwd(&self.cwd)
            .stdin(input)
            .timeout(self.timeout);
        let ran = match &self.cancel {
            Some(flag) => run_cancellable(run.cancel_on(flag.clone())),
            None => run
                .output()
                .map(|out| (out.code, out.stdout, out.stderr))
                .map_err(|e| LlmError(e.to_string())),
        };
        let (code, stdout, stderr) = match ran {
            Ok(ran) => ran,
            Err(error) => return (Err(error), String::new()),
        };
        if code != 0 {
            let error = LlmError(format!(
                "claude exited {code}: {}{}",
                stderr.trim(),
                stdout.chars().take(500).collect::<String>()
            ));
            return (Err(error), stdout);
        }
        (parse(&stdout), stdout)
    }
}

/// What goes on stdin, and whether it is an SDK message stream: the plain user text when there
/// is no image, else one user message holding the text and every image.
fn request_input(user: &str, pngs: &[String]) -> (String, bool) {
    if pngs.is_empty() {
        (user.to_owned(), false)
    } else {
        (image_input(user, pngs), true)
    }
}

/// Build one newline-delimited SDK user message, the text block followed by one image block per
/// PNG in order, without giving the CLI file access.
fn image_input(user: &str, pngs: &[String]) -> String {
    let text = serde_json::json!({"type": "text", "text": user});
    let images = pngs.iter().map(|png| {
        serde_json::json!({"type": "image", "source": {
            "type": "base64", "media_type": "image/png", "data": png
        }})
    });
    let content: Vec<serde_json::Value> = std::iter::once(text).chain(images).collect();
    let request = serde_json::json!({
        "type": "user",
        "message": {"role": "user", "content": content},
        "parent_tool_use_id": null
    });
    format!("{request}\n")
}

fn call_args(model: &str, system: &str, schema: &serde_json::Value, stream: bool) -> Vec<String> {
    let mut args: Vec<String> = [
        "-p",
        "--output-format",
        if stream { "stream-json" } else { "json" },
        "--json-schema",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    args.push(schema.to_string());
    args.extend(
        [
            "--tools",
            "",
            "--no-session-persistence",
            "--strict-mcp-config",
            "--disable-slash-commands",
            "--setting-sources",
            "project",
            "--system-prompt",
            system,
            "--model",
            model,
        ]
        .into_iter()
        .map(String::from),
    );
    if stream {
        args.extend(["--input-format", "stream-json", "--verbose"].map(String::from));
    }
    args
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

/// Read a single JSON result or the final result event of a newline-delimited stream.
pub fn parse(stdout: &str) -> Result<Completion, LlmError> {
    let value = result_value(stdout)?;
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

fn result_value(stdout: &str) -> Result<serde_json::Value, LlmError> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(stdout) {
        if value.get("type").is_some_and(|kind| kind != "result") {
            return Err(LlmError("the output stream holds no final result".into()));
        }
        return Ok(value);
    }
    let mut result = None;
    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|e| LlmError(format!("unreadable result: {e}")))?;
        if result.is_some() {
            return Err(LlmError("unexpected event after the final result".into()));
        }
        if value.get("type").and_then(|v| v.as_str()) == Some("result") {
            result = Some(value);
        }
    }
    result.ok_or_else(|| LlmError("the output stream holds no final result".into()))
}

#[cfg(test)]
#[path = "tests/claude_cli.rs"]
mod tests;
