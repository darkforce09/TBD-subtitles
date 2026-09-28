# Model call

One language-model call as the app's log window shows it: the system prompt, the message and the
schema sent, the answer that came back or why there was none, the tokens, the cost and the time.

## Contents

```text
crates/job_model/src/model_call/
├── mod.rs  `ModelExchange` and the worker's stdout line that carries it (`model-call <json>`)
└── tests/  unit tests of the line's round trip and of lines that carry no call
```

## How it works

`inference::llm::claude_cli` fills a `ModelExchange` for every call it makes and emits it as a
`tracing` event. A worker process writes each one to its stdout as a single line,
`model-call <json>` (`worker_line`); the job runner reads that line back
(`from_worker_line`) beside the `progress` lines it already reads, and the app keeps the newest
calls in memory for its log window. JSON escapes the newlines of a prompt, so a call is always one
line.

## Boundaries

- Depends on: `serde` and `serde_json`.
- Used by: `crates/inference/src/llm/claude_cli/`, `crates/pipeline/src/workers/` and
  `crates/pipeline/src/progress/`, and the app's log buffer
  (`apps/tbd_subtitles/src/core/log_buffer/`).
- Rules: a call is never written to a job's work directory or a log file (the module header); a
  line that is not a whole call is no call (`any_other_line_carries_no_call` in
  `tests/model_call.rs`).
