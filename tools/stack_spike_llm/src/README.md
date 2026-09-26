# Stack spike LLM worker source

The `stack-spike-llm` command line and the worker report it writes.

## Contents

```text
tools/stack_spike_llm/src/
├── main.rs    the binary root: the `worker` command and the local adjudication run
└── report.rs  the item outcome and the worker report in `stack-spike`'s shape, with `VmHWM`
```

## Boundaries

- Depends on: `inference::llm::mistral_rs` and `inference::model_store` (with `mistralrs`),
  `stages::adjudication`, `stages::diff_sheet::sheet`, `job_model::outputs`.
- Used by: nothing; it is the binary's source.
- Rules: the report's fields match `WorkerReport` in `tools/stack_spike/src/measure/worker.rs`
  (review).
