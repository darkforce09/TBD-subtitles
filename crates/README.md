# Library crates

The library crates of TBD-subtitles: the contracts between [stages](/documentation/glossary.md#stage),
child processes, media input through FFmpeg, subtitle formats, the inference backends, the
pipeline stages and the job runner. The app in `apps/tbd_subtitles/` composes them; the repository
tools use `child_process` alone.

## Contents

```text
crates/
├── app_icon/          the application icon, painted in code as RGBA pixels at any square size
├── child_process/     running external programs with deadlines, process-group kills and drained pipes
├── inference/         the model backends behind traits, and the model store
├── job_model/         the serde contracts between stages: jobs, stage names, stage outputs, reports
├── media_io/          FFmpeg and ffprobe as child processes: probe, PCM streaming, shot changes
├── pipeline/          the job runner: stage order, resume, worker processes, progress events
├── stages/            one module folder per pipeline stage, from probing the video to the subtitle file
└── subtitle_formats/  the cue model, the SRT, WebVTT and ASS writers, and subtitle import
```

## How it works

The crates form layers, and a crate depends only on crates of a strictly lower layer:

| Layer | Crate | Workspace dependencies |
|---|---|---|
| 0 | `job_model` | none (`serde` only) |
| 0 | `child_process` | none (`libc` only) |
| 0 | `app_icon` | none (the standard library only) |
| 1 | `media_io` | `child_process`, `job_model` |
| 1 | `inference` | `child_process`, `job_model` |
| 1 | `subtitle_formats` | `job_model` |
| 2 | `stages` | `media_io`, `inference`, `subtitle_formats`, `job_model` |
| 3 | `pipeline` | `stages`, `child_process`, `job_model` |
| 4 | `tbd_subtitles` (the app) | `pipeline`, `job_model` |

The layers follow the flow of a job, which the stage code, not written yet, fills in: the app
hands a job to `pipeline`, which walks the stages in the order `job_model::StageName::ALL` gives. A CPU stage runs
in process; a stage that loads a GPU model or the language model runs as a
[worker process](/documentation/glossary.md#worker-process), the app binary started again as
`tbd-subtitles worker <stage> <job dir>` through `child_process`. Every stage reads its inputs from
the job's [work directory](/documentation/glossary.md#work-directory) and writes one typed output
there, in `job_model` types; `media_io`, `inference` and `subtitle_formats` do the media, model
and file work beneath them.

Two crates hold working code: `child_process`, complete and tested, and the stage names in
`job_model`. Every other crate declares its module folders, each with a one-line header saying what
it is for; their code is not written yet.

## Getting started

Run these from the repository root:

```bash
cargo build --workspace                    # every crate, the app and the repository tools
cargo test -p child_process -p job_model   # the unit tests here: 20 and 5, about 3 s in all
cargo gates crate-layering                 # each crate depends only on crates of a lower layer
```

The `child_process` tests run `sh`, `cat`, `sleep` and `seq`, and one of them waits 2.5 s to
prove a killed process group left no grandchild behind. The other crates build and run no tests.

## Boundaries

- Depends on: `serde` and `libc` from crates.io, and nothing else yet. `child_process` is the one
  way these crates start a program.
- Used by: the app in `apps/tbd_subtitles/`, through `pipeline` and `job_model`; the repository
  tools, through `tools/verification_core/`, which depends on `child_process`.
- Rules:
  - a crate depends only on crates of a lower layer, and a repository tool only on the crates the
    tool table allows it, which for `tools/verification_core/` is `child_process` alone
    (`cargo gates crate-layering`, with both tables in `tools/repo_gates/src/layout.rs`);
  - production files stay under 500 lines and test files under 1000 (`cargo gates file-length`).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md) — processes, crates and the
  job work directory.
- [Pipeline](/documentation/architecture/pipeline.md) — what each stage reads, does and writes.
- [Coding standards](/documentation/standards/coding_standards.md#layering) — the layering and the
  rules for every crate.
