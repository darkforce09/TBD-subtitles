# Language-model call log

Each language-model call, logged: why it was made, one summary line, and the whole exchange (the
prompt, the message, the schema and the answer) for the app's log window.

## Contents

```text
crates/inference/src/llm/call_log/
├── mod.rs  `purpose` and its guard, `log_call`, `exchange`, `without_image_data`, `Sent` and `EXCHANGE_TARGET`
└── tests/  unit tests of the purpose stack, and of an answered and a failed call's exchange
```

## How it works

A stage says why it calls with `purpose("words of the batch from U0012")`; the purpose lasts
while the guard lives, on that thread only, and an inner purpose hides the outer until it ends.
After every call a backend calls `log_call`, which numbers the call `<pid>-<n>` and emits:

- the whole `job_model::model_call::ModelExchange` as a `trace` event with target
  `model_exchange` and one field, `exchange`, holding its JSON; it is built only when a
  subscriber wants it (`tracing::enabled!`), so a command-line run pays nothing;
- one summary line naming the selected local or Claude model: `info` when answered ("model sonnet · words of the batch from U0012: 212
  lines answered in 41.3 s, 18234 tokens in, 2210 out, $0.0874"), `warn` when not, with the
  call's number in its `call` field. It never holds the prompt or the answer.

The app routes the exchange to its log window only: its stderr and log-file outputs drop the
`model_exchange` target, and a worker process writes the exchange to its stdout, where the job
runner picks it up.
An image request keeps its text; every base64 image block in the logged message is replaced by its
size, so a keyframe request never puts megabytes of pixels into the log window.

## Boundaries

- Depends on: `job_model::model_call`, `serde_json` and `tracing`.
- Used by: `crates/inference/src/llm/claude_cli/`; the stages that set purposes
  (`crates/stages/src/adjudication/` and `crates/stages/src/fix_it/`).
- Rules: the summary never holds the prompt or the answer, and a purpose belongs to its thread
  (`a_purpose_belongs_to_its_thread` in `tests/call_log.rs`).
