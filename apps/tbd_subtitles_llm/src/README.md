# Local worker source

Command-line composition for the local visual translation worker.

## Contents

```text
apps/tbd_subtitles_llm/src/
├── logging.rs  worker diagnostics and model exchanges
└── main.rs  command parsing and pipeline worker dispatch
```

## How it works

The command delegates model work and measurements to the pipeline and prints failures to stderr.

## Boundaries

- Depends on: `pipeline`, `job_model`, `clap`.
- Used by: the `tbd-subtitles-llm` executable.
- Rules: only the local-model placement is accepted.

## Related documentation

- [Worker crate](/apps/tbd_subtitles_llm/) — build and invocation.
