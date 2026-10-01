# Pipeline stages source

The `stages` library: one module folder per stage, from probing the video to writing the subtitle
file and the localized video. Each module holds the stage's own work on typed values; the job
runner in `crates/pipeline/` reads its inputs from the job's store, calls it, and stores what it
returns. Large media (the mix, the stems, crops, stills, masks, plates and patches) stay files in
the job's work directory, which the stages read and write by path.

## Contents

```text
crates/stages/src/
├── adjudication/   the language model settles the sheet, re-asks the unsure, chooses sound cues
├── alignment/      forced alignment of the final text against the vocal stem, with fallbacks
├── asr/            each speech engine over the chunk plan: words with times and confidences
├── cues/           aligned words and sound cues laid out as cues snapped to frames and shot changes
├── diff_sheet/     every engine's words aligned to the backbone engine's, as the sheet to adjudicate
├── fix_it/         on a finished job, a stronger model fixes the flagged lines in three passes
├── lib.rs          the crate root: the module list and the crate header
├── localize/       composed English patches blended over every frame and encoded as the localized video
├── onscreen_text/  visible writing detected, read, tracked, translated, replaced and typeset as ASS
├── output/         the subtitle files written beside the video, backing up the files they replace
├── probe_decode/   ffprobe the video, choose its audio track, stream the mix into the work directory
├── qc/             the cues checked against the layout, timing and coverage rules; the job report
├── separation/     the mix split into a vocal stem and a background stem
├── sound_events/   sound events on both stems, and the candidate sound cues cut from them
└── vad/            speech found in the vocal stem, and the chunk plan every engine transcribes
```

## How it works

```text
probe_decode ─▶ separation ─▶ vad ─▶ asr ─▶ diff_sheet ─▶ sound_events ─▶ adjudication
                                                                              │
  onscreen_text ◀─ cues ◀─ alignment ◀────────────────────────────────────────┘
        │
        └─▶ qc ─▶ output ─▶ localize (the localized_video stage)
```

The arrows are the run order of `StageName::ALL`; the job runner runs each stage as one or more
steps of `StepName::ALL`. A stage reads what the stages before it stored: `diff_sheet` reads
every engine's words from `asr`, `adjudication` reads the diff sheet and the sound-event
candidates, `alignment` reads the adjudicated text, and so on down to `output`; the runner reads
each from the job's store and passes it in, so no stage opens the store.

`probe_decode/` probes and decodes the mix; `separation/` holds the stage driver and its
resampler; `vad/` the detector run, the regions and the chunk plan; `asr/` the engine trait and
the run over the plan; `diff_sheet/` the word alignment and the sheet; `sound_events/` the
windowed tagging, event cutting and the sound-cue candidates; `adjudication/` the model calls and
their checks, the re-decode of unsure lines, the sound-cue choice and the glossary; `alignment/`
the blocks, the CTC Viterbi aligner, its checks and its fallbacks; `cues/` the layout and timing
passes; `onscreen_text/` detection, reading, tracking, translation, review and typesetting of
visible writing, and in `replace/` the stroke masks, inpainting, lettering and read-back check;
`qc/` the checks and `report.md`; `output/` the installation beside the video; `localize/` the
blend of the composed patches over every frame and the encode.

`fix_it/` is not a stage of the run: it works on a finished job when the owner presses Fix It,
reading the quality check's findings and writing corrections that a correction run then times.

## Public surface

- One public module per stage, named as the stage is named on the command line, for the step
  tasks in `crates/pipeline/src/tasks/`:
  - `probe_decode`: `probe_and_decode`, `pick_track`, `Decoded`;
  - `separation`: `separate` and its request, summary and error types;
  - `vad`: `score_file`, `plan` and `VadSettings`;
  - `asr`: `SpeechEngine` and `transcribe_plan`;
  - `diff_sheet`: `align` and `sheet::{build, heard_spans}`;
  - `sound_events`: `score_stem`, `events`, `classes` and `candidates::candidates`;
  - `adjudication`: `adjudicate_concurrently`, `checks`, `redecode`, `sound_cues` and `glossary`;
  - `alignment`: `align_words_ctc`, `blocks::kept`, `run::{align_all, realign_utterance,
    WordAligner}`;
  - `cues`: `build` and `FrameRules`;
  - `onscreen_text`: `detect::scan`, `read::read`, `track::track`, `translate::translate_known`,
    `review::apply`, `unify::unify`, `typeset::events`, and `replace::{mask, inpaint, compose,
    verify}` with `replace::source::FfmpegRegions`;
  - `qc`: `check`, `QcInput` and `markdown::render`;
  - `output`: `install`, `install_localized_subtitles`, `subtitle_path` and
    `localized_video_path`.
- `fix_it`: `run`, `items::asks_about` and `changed_words`, for `crates/pipeline/src/fix_it/` and
  the window.
- `localize`: `render`, `RenderRequest`, `Rendered` and `frame_format`, for the localized-video
  step in `crates/pipeline/src/tasks/localized.rs`; `motion::Motion` and `colour::Conversion`
  for the composition and read-back tasks.

## Boundaries

- Depends on: `media_io` in `probe_decode/`, `separation/`, `vad/`, `asr/`, `sound_events/`,
  `onscreen_text/` and `localize/`; `inference` in `separation/`, `asr/`, `sound_events/`,
  `adjudication/`, `fix_it/` and `onscreen_text/`; `earshot` in `vad/`; `subtitle_formats` in
  `cues/`, `qc/` and `onscreen_text/`; `job_model` throughout.
- Used by: `crates/pipeline/src/tasks/`, `crates/pipeline/src/fix_it/`,
  `crates/pipeline/src/runner/mod.rs` and `crates/pipeline/src/report/mod.rs`; the app's job
  settings (the glossary) and job report (Fix It's items and the subtitle path); the stack spike
  tools in `tools/stack_spike/`, `tools/stack_spike_ggml/` and `tools/stack_spike_llm/`; and
  `tools/visual_validation/`.
- Rules: each stage module is named exactly as its stage's `StageName::as_str` name (`fix_it/`
  runs outside the stages, and `localize/` is the `localized_video` stage), and a stage's output
  is complete or absent, never partial (the crate header in `lib.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#stage-flow) — the stage flow and every stage in
  detail.
- [Pipeline](/documentation/architecture/pipeline.md#steps-and-processes) — the steps each stage
  runs as, and where each runs.
