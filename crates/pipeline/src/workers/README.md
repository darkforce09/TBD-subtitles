# Worker processes

Starting a GPU stage as a `worker` subprocess of the app binary, one at a time, and reading its
result. The module's code is not written yet; `mod.rs` holds only its header.

## Contents

```text
crates/pipeline/src/workers/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/pipeline/src/lib.rs` declares it as a public module.
- Rules: one GPU worker runs at a time (the crate header in `crates/pipeline/src/lib.rs`); the app's
  `worker` subcommand accepts only the stages for which `StageName::runs_in_worker` is true
  (`worker_accepts_gpu_stages_only` in `apps/tbd_subtitles/src/cli/tests/cli.rs`, and
  `only_model_stages_run_in_a_worker` in `crates/job_model/src/stage/tests/stage_name.rs`).

## Related documentation

- [Glossary: worker process](/documentation/glossary.md#worker-process) — the term.
- [Decisions](/documentation/decisions.md) — why each GPU stage runs in its own worker process.
