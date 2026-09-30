# Log buffer

The log window's memory and the `tracing` layers that fill it: the newest lines and model calls of
the running app, each with the video and step it belongs to, and the path a worker's model calls
take to reach it.

## Contents

```text
apps/tbd_subtitles/src/core/log_buffer/
├── layer.rs           `ConsoleLayer`: each event a line or a kept call, with its video and step
├── mod.rs             `LogBuffer`, `LogLine`, `KeptCall`, `Fresh`, `CAPACITY` and `CALL_CAPACITY`
├── tests/             unit tests of the rings, the layer's context and routing, and the worker line
├── worker_channel.rs  `WorkerChannelLayer`: a worker's model calls sent to the runner as frames
└── worker_line.rs     a worker's own `tracing` line read back into its level, target, text and call
```

## How it works

`LogBuffer` keeps two rings under their own locks: the newest 20,000 lines (`CAPACITY`) and the
newest 500 model calls (`CALL_CAPACITY`). Each item has a sequence number that only grows, so the
log window reads only what is newer than what it has (`since`, `calls_since`); Clear empties one
ring and keeps its numbering.

`ConsoleLayer` sits over the registry in the window's run. When a span opens it keeps the span's
`video` and `step` fields (the job runner opens `job{video}`, the pipeline `step{step}`, Fix It
`step{step="fix_it"}`); each event then takes its own `video`, `step` and `call` fields first and
the nearest span's otherwise, so a line knows which video and step it is about even when it was
logged on a program's drain thread. An event with the target `model_exchange` carries one model
call as JSON and becomes a kept call, never a line; a broken one is dropped. A `child_process`
line that is a worker's own `tracing` line (`tbd-subtitles[4242] 2026-…Z  INFO pipeline::tasks:
text call=4242-3`) is read back (`worker_line`), so it shows with the worker's level and source
and links to its call.

A worker process has no window: `WorkerChannelLayer` sends each `model_exchange` event's JSON to
the job runner as one `ModelCall` frame of the worker channel (`worker_channel::worker`), which
the runner reads beside the progress frames and logs again in the app, under the job's and the
step's spans.

## Boundaries

- Depends on: `tracing`, `tracing-subscriber` (the registry and `LookupSpan`), `serde_json`,
  `job_model::model_call`, `worker_channel::worker` and
  `inference::llm::call_log::EXCHANGE_TARGET`.
- Used by: `crate::core::logging`, which installs the layers; `crate::application`, whose
  environment holds the buffer; `crate::log_console`, which reads `LogLine` and `KeptCall`.
- Rules: a model call is never a line and an event's own context wins over its spans'
  (`a_model_call_is_kept_as_a_call_under_its_step_and_never_as_a_line`,
  `an_event_s_own_context_and_call_win_and_leave_its_text` in `tests/layer.rs`); a program's line
  that is not a worker's `tracing` line is left alone (`any_other_program_line_is_left_alone` in
  `tests/worker_line.rs`).
