# Pipeline stages source

The `stages` library: one module folder per stage, from probing the video to writing the subtitle
file. Each module holds the stage's own work on typed values; the job runner in `crates/pipeline/`
reads its inputs from the work directory, calls it, and writes what it returns.

## Contents

```text
crates/stages/src/
├── onscreen_text/  sampled text detection, local OCR, keyframe geometry checks, translation and ASS presentation
├── adjudication/  the language model settles the sheet, re-asks the unsure, chooses sound cues
├── alignment/     forced alignment of the final text against the vocal stem, with fallbacks
├── asr/           each speech engine over the chunk plan: words with times and confidences
├── cues/          aligned words and sound cues laid out as cues snapped to frames and shot changes
├── diff_sheet/    every engine's words aligned to the backbone engine's, as the sheet to adjudicate
├── fix_it/        on a finished job, a stronger model fixes the flagged lines in three passes
├── lib.rs         the crate root: the module list and the crate header
├── localize/      composed English patches blended over every frame and encoded as the localized video
├── output/        the subtitle file written beside the video, backing up the file it replaces
├── probe_decode/  ffprobe the video, choose its audio track, stream the mix into the work directory
├── qc/            the cues checked against the layout, timing and coverage rules; the job report
├── separation/    the mix split into a vocal stem and a background stem
├── sound_events/  sound events on both stems, and the candidate sound cues cut from them
└── vad/           speech found in the vocal stem, and the chunk plan every engine transcribes
```

## How it works

```text
probe_decode ─▶ separation ─▶ vad ─▶ asr ─▶ diff_sheet ─▶ sound_events ─▶ adjudication
                                                                              │
                                output ◀─ qc ◀─ cues ◀─ alignment ◀───────────┘
```

The arrows are the run order of `StageName::ALL`; the job runner runs each stage as one or more
steps of `StepName::ALL`. A stage reads what the stages before it wrote to the job's work
directory: `diff_sheet` reads every engine's words from `asr`, `adjudication` reads the diff sheet
and the sound-event candidates, `alignment` reads the adjudicated text, and so on down to `output`.

`probe_decode/` probes and decodes the mix; `separation/` holds the stage driver and its
resampler; `vad/` the detector run, the regions and the chunk plan; `asr/` the engine trait and
the run over the plan; `diff_sheet/` the word alignment and the sheet; `sound_events/` the
windowed tagging, event cutting and the sound-cue candidates; `adjudication/` the model calls and
their checks, the re-decode of unsure lines, the sound-cue choice and the glossary; `alignment/`
the blocks, the CTC Viterbi aligner, its checks and its fallbacks; `cues/` the layout and timing
passes; `qc/` the checks and `report.md`; `output/` the installation beside the video.

`fix_it/` is not a stage of the run: it works on a finished job when the owner presses Fix It,
reading the quality check's findings and writing corrections that a correction run then times.

## Public surface

- One public module per stage, named as the stage is named on the command line, for the step
  tasks in `crates/pipeline/src/tasks/`:
  - `probe_decode`: `probe_and_decode`, `pick_track`, `Decoded`;
  - `separation`: `separate` and its request, summary and error types;
  - `vad`: `score_file`, `plan` and `VadSettings`;
  - `asr`: `SpeechEngine` and `transcribe_plan`;
  - `diff_sheet`: `align` and `sheet::build`;
  - `sound_events`: `score_stem`, `events`, `classes` and `candidates::candidates`;
  - `adjudication`: `adjudicate_concurrently`, `checks`, `redecode`, `sound_cues` and `glossary`;
  - `alignment`: `align_words_ctc`, `blocks::kept`, `run::{align_all, WordAligner}`;
  - `cues`: `build` and `FrameRules`;
  - `qc`: `check`, `QcInput` and `markdown::render`;
  - `output`: `install` and `subtitle_path`.
- `fix_it`: `run` and `items::asks_about`, for `crates/pipeline/src/fix_it/` and the window.
- `localize`: `render`, `RenderRequest`, `Rendered` and `frame_format`, for the localized-video
  step in `crates/pipeline/src/tasks/localized.rs`.

## Boundaries

- Depends on: `media_io` in `probe_decode/`, `separation/`, `vad/`, `asr/` and `sound_events/`;
  `inference` in `separation/`, `asr/`, `sound_events/` and `adjudication/`; `earshot` in `vad/`;
  `subtitle_formats::cue` in `cues/` and `qc/`; `job_model` throughout.
- Used by: `crates/pipeline/src/tasks/`, `crates/pipeline/src/graph/mod.rs`,
  `crates/pipeline/src/runner/mod.rs` and `crates/pipeline/src/report/mod.rs`; the app's `process`
  subcommand (the glossary); the stack spike tools in `tools/stack_spike/`,
  `tools/stack_spike_ggml/` and `tools/stack_spike_llm/`.
- Rules: each stage module is named exactly as its stage's `StageName::as_str` name (`fix_it/`
  runs outside the stages, and `localize/` is the `localized_video` stage), and a stage's output is complete or absent, never partial (the crate
  header in `lib.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#stage-flow) — the stage flow and every stage in
  detail.
- [Pipeline](/documentation/architecture/pipeline.md#steps-and-processes) — the steps each stage
  runs as, and where each runs.
