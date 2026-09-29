# Local translation worker

The `tbd-subtitles-llm` executable isolates mistral.rs from ONNX Runtime and ggml.

## Contents

```text
apps/tbd_subtitles_llm/
├── Cargo.toml  worker manifest and optional CUDA backend
└── src/        worker command
```

## How it works

The pipeline starts `tbd-subtitles-llm worker text_translate <job-dir>` while holding its GPU lock. The worker loads one local model, writes resumable visual translations, reports time and memory, then exits.

## Getting started

Build with `cargo build -p tbd_subtitles_llm --features mistralrs` under the CUDA environment described in the development runbook. A build without this feature reports an actionable error.

## Configuration

The job record selects models, glossary and Claude fallback. The worker uses the shared GPU lock and persisted Claude call cap.

## Public surface

- `worker <step> <job-dir>` uses the standard pipeline worker protocol.

## Boundaries

- Depends on: `pipeline`, `job_model` and `clap`.
- Used by: the pipeline runner and AppImage packager.
- Rules: this worker never initializes ONNX Runtime or ggml.

## Related documentation

- [Development environment](/documentation/runbooks/development_environment.md) — CUDA build configuration.
