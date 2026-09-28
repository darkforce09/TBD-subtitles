# Pipeline stages

The `stages` crate: one module folder per pipeline [stage](/documentation/glossary.md#stage), from
probing the video to writing the subtitle file. Each stage turns typed inputs, read from the job's
work directory by the job runner in `crates/pipeline/`, into one typed output that the runner
writes back there.

## Contents

```text
crates/stages/
├── Cargo.toml  the `stages` library package: `media_io`, `inference`, `subtitle_formats`, `job_model`
└── src/        one module folder per stage, from probe and decode to output
```

## How it works

The job runner runs each stage as one or more steps (`job_model::StepName`), in process when the
step does pure work on files and inside a
[worker process](/documentation/glossary.md#worker-process) when it loads a model, starts FFmpeg
or runs the `claude` CLI; `crates/pipeline/src/graph/mod.rs` holds the placement. The run order is
that of `job_model::StageName`:

| # | Module | Runs in | Does |
|---|---|---|---|
| 1 | `probe_decode` | a worker (FFmpeg) | probes the video and streams its audio; the shot scan runs beside it |
| 2 | `separation` | a worker | splits the mix into a vocal and a background [stem](/documentation/glossary.md#stem) |
| 3 | `vad` | the runner | finds speech and plans the chunks every engine transcribes |
| 4 | `asr` | a worker per engine | runs each speech engine over the chunks |
| 5 | `diff_sheet` | the runner | aligns the engines' words and writes the [diff sheet](/documentation/glossary.md#diff-sheet) |
| 6 | `sound_events` | a worker | detects sound events on the stems |
| 7 | `adjudication` | workers | settles the sheet, re-decodes the unsure lines, chooses the sound cues |
| 8 | `alignment` | a worker | force-aligns the final text against the vocal stem |
| 9 | `cues` | the runner | lays the words and sound cues out as subtitle cues |
| 10 | `qc` | the runner | checks the cues and renders the job report |
| 11 | `output` | the runner | installs the subtitle file beside the video |

Media work goes through `media_io`, models through `inference`, and cues through
`subtitle_formats`. `probe_decode` streams the mix to 16 kHz; `separation` streams it through a
separation model and writes the two stems; `vad` scores a stem with earshot and plans the chunks;
`asr` runs any `SpeechEngine` over the plan; `diff_sheet` lines the engines' words up into the
sheet; `sound_events` turns tagger scores into events and events into sound-cue candidates;
`adjudication` has a language model settle the sheet, hear the unsure lines again and choose the
sound cues, and checks every answer; `alignment` times the final words block by block with
fallbacks; `cues` lays them out on frames; `qc` checks the result and writes the report; `output`
installs the file. `src/README.md` describes each module.

## Getting started

Run these from the repository root:

```bash
cargo build -p stages   # the stages, with the crates beneath them
cargo test -p stages    # 111 unit tests, well under a second; no model, GPU or FFmpeg needed
```

## Configuration

- The Cargo feature `crispasr` (off by default) adds the Whisper `SpeechEngine`, through
  `inference/crispasr`; building it needs cmake and the CUDA toolkit
  (`src/asr/engines.rs`). `crates/pipeline/` passes it on for the `tbd-subtitles-ggml` binary.
- The stages read no environment variable or file of their own; every setting reaches them as an
  argument from `job_model::job::JobSettings`.

## Public surface

- The library `stages`, with one public module per stage: `probe_decode`, `separation`, `vad`,
  `asr`, `diff_sheet`, `sound_events`, `adjudication`, `alignment`, `cues`, `qc` and `output`,
  each offering the functions its step task in `crates/pipeline/src/tasks/` calls
  (`src/README.md` lists them).
- `adjudication::glossary`: the built-in One Piece glossary and the glossary file reader, for the
  app's `process` subcommand.
- No binary.

## Boundaries

- Depends on: `media_io` (probe, PCM streams, stem files), `inference` (the ONNX models, the
  speech engines and the language models), `subtitle_formats` (the cue model), `job_model` (the
  output types); `earshot` (in `vad`), `soundevents-dataset` (in `sound_events`), `serde` and
  `serde_json` (in `adjudication`), `tracing` (Fix It's call threads keep the caller's span).
- Used by: `crates/pipeline/` (every step task, the step graph, the runner and the report);
  `apps/tbd_subtitles/` (the glossary); `tools/stack_spike/`, `tools/stack_spike_ggml/` and
  `tools/stack_spike_llm/`.
- Rules:
  - the crate sits in layer 2 and depends only on lower layers (`cargo gates crate-layering`,
    layer table in `tools/repo_gates/src/layout.rs`);
  - every GPU step runs in a worker, and Whisper alone in the ggml binary
    (`gpu_steps_run_in_workers_and_whisper_alone_in_the_ggml_binary` in
    `crates/pipeline/src/graph/tests/graph.rs`);
  - a stage's output is complete or absent, never partial, and only the output stage writes
    beside the video (the crate header in `crates/stages/src/lib.rs`);
  - no stage invents a word that no speech engine heard (the crate header, held for the language
    model by the checks in `crates/stages/src/adjudication/checks.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — what each stage reads, does and writes,
  and the steps it runs as.
- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the file
  each stage writes.
