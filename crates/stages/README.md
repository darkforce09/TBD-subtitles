# Pipeline stages

The `stages` crate: one module folder per pipeline [stage](/documentation/glossary.md#stage), from
probing the video to writing the subtitle file. Each stage reads its inputs from the job's work
directory and writes one typed output there. The stage modules are not written yet.

## Contents

```text
crates/stages/
├── Cargo.toml  the `stages` library package: `media_io`, `inference`, `subtitle_formats`, `job_model`
└── src/        one module folder per stage, from probe and decode to output
```

## How it works

The job runner in `crates/pipeline/` calls a stage in process when it runs on the CPU, and inside
a [worker process](/documentation/glossary.md#worker-process) when it loads a GPU model or the
language model. The run order and the worker split are those of `job_model::StageName`:

| # | Module | Runs in | Does |
|---|---|---|---|
| 1 | `probe_decode` | the runner | probes the video, streams its audio, scans its shot changes |
| 2 | `separation` | a worker | splits the mix into a vocal and a background [stem](/documentation/glossary.md#stem) |
| 3 | `vad` | the runner | finds speech and plans the chunks every engine transcribes |
| 4 | `asr` | a worker | runs each speech engine over the chunks |
| 5 | `diff_sheet` | the runner | aligns the engines' words and writes the [diff sheet](/documentation/glossary.md#diff-sheet) |
| 6 | `sound_events` | a worker | detects sound events on the stems |
| 7 | `adjudication` | a worker | lets the language model settle each disagreement and choose sound cues |
| 8 | `alignment` | a worker | force-aligns the final text against the vocal stem |
| 9 | `cues` | the runner | lays the words and sound cues out as subtitle cues |
| 10 | `qc` | the runner | checks the cues and writes the job report |
| 11 | `output` | the runner | writes the subtitle file beside the video |

Media work goes through `media_io`, models through `inference`, and cues and files through
`subtitle_formats`. Each module holds only its header; `src/README.md` describes each.

## Getting started

Run these from the repository root:

```bash
cargo build -p stages   # the module declarations, with the crates beneath them
cargo test -p stages    # runs 0 tests: no module holds code yet
```

## Configuration

None: the crate reads no setting.

## Public surface

- The library `stages`, with one public module per stage: `probe_decode`, `separation`, `vad`,
  `asr`, `diff_sheet`, `sound_events`, `adjudication`, `alignment`, `cues`, `qc` and `output`;
  they hold no items yet.
- No binary.

## Boundaries

- Depends on: `media_io`, `inference`, `subtitle_formats` and `job_model`, declared in `Cargo.toml`
  and not called yet.
- Used by: `crates/pipeline/`, which declares it as a dependency.
- Rules:
  - the crate sits in layer 2 and depends only on lower layers (`cargo gates crate-layering`,
    layer table in `tools/repo_gates/src/layout.rs`);
  - which stages run in a worker is set by `StageName::runs_in_worker`
    (`only_model_stages_run_in_a_worker` in `crates/job_model/src/stage/tests/stage_name.rs`);
  - a stage's output is complete or absent, never partial, and only the output stage writes
    beside the video (the crate header in `crates/stages/src/lib.rs`; no test holds these yet).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — what each stage reads, does and writes.
- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the file
  each stage writes.
