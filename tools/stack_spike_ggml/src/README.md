# Stack spike ggml worker source

The `stack-spike-ggml` command line, its items and the worker report it writes.

## Contents

```text
tools/stack_spike_ggml/src/
├── items.rs   the ggml items: Whisper large-v3 on the mix and the RoFormer stem, turbo on the mix
├── main.rs    the binary root: the `worker` command
└── report.rs  the item outcome and the worker report in `stack-spike`'s shape, with `VmHWM`
```

## Boundaries

- Depends on: `inference::ggml::crispasr` and `inference::model_store` (with `crispasr`),
  `stages::asr`, `job_model::outputs`.
- Used by: nothing; it is the binary's source.
- Rules: the report's fields match `WorkerReport` in `tools/stack_spike/src/measure/worker.rs`
  (review).
