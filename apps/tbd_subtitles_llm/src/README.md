# Local worker source

Command-line composition for the local visual translation worker.

## Contents

```text
apps/tbd_subtitles_llm/src/
├── logging.rs  worker diagnostics to stderr, model exchanges to the runner as frames
└── main.rs     command parsing and pipeline worker dispatch
```

## How it works

The command delegates model work and measurements to the pipeline and prints failures to stderr. `logging.rs` installs the subscriber: diagnostics go to stderr, and `WorkerChannelLayer` sends each model call's JSON through `worker_channel::worker::model_call`, which the pipeline's `worker_main` installs before the step starts.

## Boundaries

- Depends on: `pipeline`, `job_model`, `inference`, `worker_channel`, `tracing`,
  `tracing-subscriber`, `clap`.
- Used by: the `tbd-subtitles-llm` executable.
- Rules: only the local-model placement is accepted.

## Related documentation

- [Worker crate](/apps/tbd_subtitles_llm/) — build and invocation.
