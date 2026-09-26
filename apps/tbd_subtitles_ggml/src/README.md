# TBD Subtitles ggml worker source

The `tbd-subtitles-ggml` command line: one `worker` command that runs a Whisper step of a job
through the pipeline crate.

## Contents

```text
apps/tbd_subtitles_ggml/src/
├── main.rs  the binary root: the `worker` command, its step check, the exit code
└── tests/   parsing, the Whisper-only step check, and the refusal without CrispASR
```

## Boundaries

- Depends on: `pipeline::graph` (`placement`, `Binary`, `Placement`) and
  `pipeline::tasks::worker_main`; `job_model::StepName`; `clap` and `anyhow`.
- Used by: nothing links it; the job runner in `crates/pipeline/` starts the binary.
- Rules: `worker` accepts only the steps placed in `Binary::Ggml`
  (`only_the_whisper_steps_are_accepted` in `tests/cli.rs`), and a build without the `crispasr`
  feature refuses to run (`a_build_without_crispasr_refuses_to_run`).
