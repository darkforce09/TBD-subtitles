# Pipeline stages

The `stages` crate: one module folder per pipeline [stage](/documentation/glossary.md#stage), from
probing the video to writing the subtitle file and the localized video. Each stage turns typed
inputs, read from the job's store by the job runner in `crates/pipeline/`, into typed outputs
that the runner stores there; large media (the mix, the stems, crops, masks, plates and patches)
are files the stages read and write in the job's work directory.

## Contents

```text
crates/stages/
├── Cargo.toml  the `stages` library package: `media_io`, `inference`, `subtitle_formats`, `job_model`
└── src/        one module folder per stage, from probe and decode to the localized video
```

## How it works

The job runner runs each stage as one or more steps (`job_model::StepName`), in process for the
steps that do pure work on values and inside a
[worker process](/documentation/glossary.md#worker-process) for the rest, which load a model,
start FFmpeg, run the `claude` CLI or must stay cancellable; `crates/pipeline/src/graph/mod.rs`
holds the placement. The run order is that of `job_model::StageName`:

| # | Module | Runs in | Does |
|---|---|---|---|
| 1 | `probe_decode` | a worker (FFmpeg) | probes the video and streams its audio; the shot scan runs beside it |
| 2 | `separation` | a worker | splits the mix into a vocal and a background [stem](/documentation/glossary.md#stem) |
| 3 | `vad` | the runner | finds speech and plans the chunks every engine transcribes |
| 4 | `asr` | a worker per engine | runs each speech engine over the chunks |
| 5 | `diff_sheet` | the runner | aligns the engines' words and builds the [diff sheet](/documentation/glossary.md#diff-sheet) |
| 6 | `sound_events` | a worker | detects sound events on the stems |
| 7 | `adjudication` | workers | settles the sheet, re-decodes the unsure lines, chooses the sound cues |
| 8 | `alignment` | workers | force-aligns the final text against the vocal stem, and re-times corrected lines |
| 9 | `cues` | the runner | lays the words and sound cues out as subtitle cues |
| 10 | `onscreen_text` | workers, review in the runner | detects, reads, tracks, translates, replaces and typesets visible writing |
| 11 | `qc` | the runner | checks the cues and renders the job report |
| 12 | `output` | the runner | installs the subtitle files beside the video |
| 13 | `localize` | a worker (FFmpeg) | blends the composed English patches into every frame and encodes the localized video |

Media work goes through `media_io`, models through `inference`, and cues through
`subtitle_formats`. `probe_decode` streams the mix to 16 kHz; `separation` streams it through a
separation model and writes the two stems; `vad` scores a stem with earshot and plans the chunks;
`asr` runs any `SpeechEngine` over the plan; `diff_sheet` lines the engines' words up into the
sheet; `sound_events` turns tagger scores into events and events into sound-cue candidates;
`adjudication` has a language model settle the sheet, hear the unsure lines again and choose the
sound cues, and checks every answer; `alignment` times the final words block by block with
fallbacks; `cues` lays them out on frames; `onscreen_text` turns visible Japanese into ASS events
and, for the localized video, into composed English patches; `qc` checks the result and writes
the report; `output` installs the files; `localize` encodes the localized video. Fix It
(`fix_it`) works on a finished job outside the run. `src/README.md` describes each module.

## Getting started

Run these from the repository root:

```bash
cargo build -p stages   # the stages, with the crates beneath them
cargo test -p stages    # 535 unit tests in about 10 s; no model, GPU or FFmpeg needed
```

Eleven further tests are ignored by default: three need FFmpeg, six the `latin-fonts` model, and
two a job's dumped outputs named by environment variables.

## Configuration

- The Cargo feature `crispasr` (off by default) adds the Whisper `SpeechEngine`, through
  `inference/crispasr`; building it needs cmake and the CUDA toolkit
  (`src/asr/engines.rs`). `crates/pipeline/` passes it on for the `tbd-subtitles-ggml` binary.
- The stages read no environment variable or settings file of their own; every setting reaches
  them as an argument from `job_model::job::JobSettings`.

## Public surface

- The library `stages`, with one public module per stage: `probe_decode`, `separation`, `vad`,
  `asr`, `diff_sheet`, `sound_events`, `adjudication`, `alignment`, `cues`, `onscreen_text`, `qc`,
  `output` and `localize`, each offering the functions its step task in
  `crates/pipeline/src/tasks/` calls (`src/README.md` lists them).
- `fix_it`: one Fix It run, for `crates/pipeline/src/fix_it/`, and its items and changed words for
  the app's job report.
- `adjudication::glossary`: the built-in One Piece glossary and the glossary file reader, for the
  app's job settings.
- No binary.

## Boundaries

- Depends on: `media_io` (probe, PCM streams, stem files, frame streams, region crops and the
  encode), `inference` (the ONNX models, the OCR, the speech engines and the language models),
  `subtitle_formats` (the cue model), `job_model` (the output types); `earshot` (in `vad`),
  `soundevents-dataset` (in `sound_events`), `serde` and `serde_json` (the language models' answers
  and the visual caches), `image`, `imageproc`, `nalgebra`, `fontdb`, `ttf-parser`, `tiny-skia`
  and `base64` (in `onscreen_text` and `localize`), `tracing` (Fix It's call threads keep the
  caller's span).
- Used by: `crates/pipeline/` (every step task, Fix It, the runner and the report);
  `apps/tbd_subtitles/` (the glossary, Fix It's items and the subtitle path);
  `tools/stack_spike/`, `tools/stack_spike_ggml/`, `tools/stack_spike_llm/` and
  `tools/visual_validation/`.
- Rules:
  - the crate sits in layer 2 and depends only on lower layers (`cargo gates crate-layering`,
    layer table in `tools/repo_gates/src/layout.rs`);
  - every GPU step runs in a worker, and Whisper alone in the ggml binary
    (`gpu_steps_run_in_workers_and_whisper_alone_in_the_ggml_binary` in
    `crates/pipeline/src/graph/tests/graph.rs`);
  - no stage opens the job's store: the runner passes every stored input in and stores every
    output (the crate has no `redb` dependency);
  - a stage's output is complete or absent, never partial, and only the output stage, and
    `localize` at the path its task hands it, write beside the video (the crate header in
    `crates/stages/src/lib.rs`);
  - no stage invents a word that no speech engine heard (the crate header, held for the language
    model by the checks in `crates/stages/src/adjudication/checks.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — what each stage reads, does and writes,
  and the steps it runs as.
- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the job's
  store and the files each stage writes.
