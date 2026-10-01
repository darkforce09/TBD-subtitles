# Local translation worker

The `tbd-subtitles-llm` executable isolates mistral.rs from ONNX Runtime and ggml.

## Contents

```text
apps/tbd_subtitles_llm/
├── Cargo.toml  worker manifest and optional CUDA backend
└── src/        worker command
```

## How it works

The pipeline starts `tbd-subtitles-llm worker text_translate <job-dir>` without taking the GPU lock, and samples the worker's VRAM while it runs. `src/logging.rs` sends diagnostics to stderr and each model call's JSON to the runner as a `ModelCall` frame of the worker channel (`WorkerChannelLayer`). `pipeline::tasks::worker_main` installs the channel before mistral.rs loads, so descriptor 1 then points at stderr; the worker reads the job record and the step's inputs from the runner's frames on stdin, asks Claude per keyframe when the job allows it and loads the local model only for what Claude and the sign library leave. Just before that load it takes the GPU lock (after any waiting main-walk step) and waits for the memory the step needs, at most 10 minutes, sending each waiting line to the runner as a `Message` frame, and holds both until the model drops (`crates/pipeline/src/workers/lazy_gpu.rs`). It keeps its translation caches in the job folder, sends the translated document, its progress, time and memory as frames, then exits. It never opens the job's database.

## Getting started

Build with `cargo build -p tbd_subtitles_llm --features mistralrs` under the CUDA environment described in the development runbook. A build without this feature reports an actionable error.

## Configuration

The job record the runner sends selects the models folder, the glossary, whether Claude is asked (`claude_fallback`), the Claude model and the number of Claude calls at once. The runner names the shared GPU lock file in `TBD_SUBTITLES_GPU_LOCK`, which the worker takes only while its local model is loaded, adds the CUDA runtime to `LD_LIBRARY_PATH` and names the sign library in `TBD_SUBTITLES_LIBRARY` (`crates/pipeline/src/library/mod.rs`). The Cargo feature `mistralrs` (off by default) enables the local model backend.

## Public surface

- `worker <step> <job-dir>` speaks the worker channel (`crates/worker_channel/`) on stdout.

## Boundaries

- Depends on: `pipeline`, `job_model`, `inference` (the model-call target), `worker_channel`,
  `tracing`, `tracing-subscriber` and `clap`.
- Used by: the pipeline runner and AppImage packager.
- Rules: this worker never initializes ONNX Runtime or ggml.

## Related documentation

- [Development environment](/documentation/runbooks/development_environment.md) — CUDA build configuration.
