# Library crates

The library crates of TBD-subtitles: the contracts between [stages](/documentation/glossary.md#stage),
child processes, the worker channel, the application icon, media input through FFmpeg, subtitle
formats, the inference backends, the pipeline stages and the job runner. The three app binaries in
`apps/` compose them, and the repository tools in `tools/` use the ones their table allows.

## Contents

```text
crates/
├── app_icon/          the application icon, painted in code as RGBA pixels at any square size
├── child_process/     running external programs with deadlines, process-group kills and drained pipes
├── inference/         the model backends behind traits, the model store and the CUDA runtime lookup
├── job_model/         the contracts between stages: stage and step names, job record, outputs, reports
├── media_io/          FFmpeg and ffprobe as child processes: probe, PCM, frames, the localized video
├── pipeline/          the job runner: step order, resume, job store, workers, sign library, report
├── stages/            one module folder per pipeline stage, from probing the video to the subtitle file
├── subtitle_formats/  the cue model, the SRT, WebVTT and ASS writers, and subtitle import
└── worker_channel/    the frames a worker and the job runner exchange on pipes, and the worker's side
```

## How it works

The crates form layers, and a crate depends only on crates of a strictly lower layer:

| Layer | Crate | Workspace dependencies |
|---|---|---|
| 0 | `job_model` | none (`serde`, `rkyv` and `sha2`) |
| 0 | `child_process` | none (`libc` and `tracing`) |
| 0 | `app_icon` | none (the standard library only) |
| 0 | `worker_channel` | none (`rustix` only) |
| 1 | `media_io` | `child_process`, `job_model` |
| 1 | `inference` | `child_process`, `job_model` |
| 1 | `subtitle_formats` | `job_model` |
| 2 | `stages` | `media_io`, `inference`, `subtitle_formats`, `job_model` |
| 3 | `pipeline` | `stages`, `inference`, `media_io`, `subtitle_formats`, `child_process`, `job_model`, `worker_channel` |
| 4 | `tbd_subtitles` (the app) | `pipeline`, `stages`, `inference`, `media_io`, `child_process`, `job_model`, `worker_channel`, `app_icon` |
| 4 | `tbd_subtitles_ggml` (the Whisper worker) | `pipeline`, `job_model` |
| 4 | `tbd_subtitles_llm` (the local-model worker) | `pipeline`, `inference`, `job_model`, `worker_channel` |

The layers follow the flow of a job: the app hands a video to `pipeline`, which walks the
twenty-nine steps in the order `job_model::StepName::ALL` gives. A CPU step runs in process; a
step that loads a GPU model, runs the language model or does long visual work runs as a
[worker process](/documentation/glossary.md#worker-process), one of the three app binaries started
again as `<binary> worker <step> <job dir>` through `child_process`. Every step's documents and
record live in the job's one database, `job.redb`, in the job's
[work directory](/documentation/glossary.md#work-directory), as rkyv archives of `job_model`
types; a worker never opens it, and takes its inputs and returns its outputs as frames of
`worker_channel` on its pipes. The large media (audio streams, stills, masks, plates, patches)
stay files the rows name. `stages` holds the work of each stage, and `media_io`, `inference` and
`subtitle_formats` do the media, model and file work beneath it.

## Getting started

Run these from the repository root:

```bash
cargo build --workspace                    # every crate, the apps and the repository tools
cargo test -p job_model -p child_process   # the unit tests of two bottom-layer crates: 115 and 40
cargo gates crate-layering                 # each crate depends only on crates of a lower layer
```

The `child_process` tests run `sh`, `cat`, `sleep` and `seq`, and one of them waits 2.5 s to
prove a killed process group left no grandchild behind. Each crate's README gives its own tests.

## Boundaries

- Depends on: crates.io crates such as `serde`, `rkyv`, `redb`, `ort`, `image` and `libc`; at run
  time FFmpeg and ffprobe for `media_io`, the `claude` CLI for `inference`, and the app's worker
  binaries for `pipeline`. `child_process` is the one way these crates start a program.
- Used by: the three app binaries in `apps/`; the repository tools in `tools/`, each within the
  crates the tool table in `tools/repo_gates/src/layout.rs` allows it (`verification_core` uses
  `child_process` alone).
- Rules:
  - a crate depends only on crates of a lower layer, and a repository tool only on the crates the
    tool table allows it (`cargo gates crate-layering`, with both tables in
    `tools/repo_gates/src/layout.rs`);
  - production files stay under 500 lines and test files under 1000 (`cargo gates file-length`).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md) — processes, crates and the
  job work directory.
- [Pipeline](/documentation/architecture/pipeline.md) — what each stage reads, does and writes.
- [Coding standards](/documentation/standards/coding_standards.md#layering) — the layering and the
  rules for every crate.
