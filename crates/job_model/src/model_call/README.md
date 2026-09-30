# Model call

One language-model call as the app's log window shows it: the system prompt, the message and the
schema sent, the answer that came back or why there was none, the tokens, the cost and the time.

## Contents

```text
crates/job_model/src/model_call/
├── mod.rs  `ModelExchange`, whose JSON crosses from a worker to the app in one `ModelCall` frame
└── tests/  unit tests of the JSON round trip, of JSON that is no call and of the rkyv archive
```

## How it works

`inference::llm::claude_cli` fills a `ModelExchange` for every call it makes and emits it as a
`tracing` event. A worker process sends each one's serde JSON to the job runner as one
`ModelCall` frame of the worker channel (`crates/worker_channel/`); the runner parses the frame's
bytes back into a `ModelExchange` beside the progress frames it reads, and the app keeps the newest
calls in memory for its log window. The JSON field names are the contract between the app and its
workers, which are always built together.

## Boundaries

- Depends on: `serde` (the JSON contract) and `rkyv` (the archive); `serde_json` in the tests.
- Used by: `crates/inference/src/llm/claude_cli/`, `crates/pipeline/src/workers/frames.rs` and
  `crates/pipeline/src/progress/`, and the app's log buffer
  (`apps/tbd_subtitles/src/core/log_buffer/`).
- Rules: a call is never written to a job's work directory or a log file (the module header); a
  frame whose JSON is not a whole call is no call (`json_that_is_not_a_whole_call_is_no_call` in
  `tests/model_call.rs`, and `an_unreadable_model_call_is_a_short_message` in
  `crates/pipeline/src/workers/tests/frames.rs`).
