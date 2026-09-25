# Pipeline stages source

The `stages` library: one module folder per stage, from probing the video to writing the subtitle
file. The modules are not written yet.

## Contents

```text
crates/stages/src/
├── adjudication/  the language model settles each disagreement, chooses sound cues, flags doubt
├── alignment/     forced alignment of the final text against the vocal stem, with fallbacks
├── asr/           each speech engine over the chunk plan: words with times and confidences
├── cues/          aligned words and sound cues laid out as cues snapped to frames and shot changes
├── diff_sheet/    every engine's words aligned to the backbone engine's, as the sheet to adjudicate
├── lib.rs         the crate root: the module list and the crate header
├── output/        the subtitle file written beside the video, backing up the file it replaces
├── probe_decode/  ffprobe the video, stream its audio into the work directory, scan shot changes
├── qc/            the cues checked against the layout, timing and coverage rules; the job report
├── separation/    the mix split into a vocal stem and a background stem
├── sound_events/  sound events on both stems, window scores turned into candidate cues
└── vad/           speech found in the vocal stem, and the chunk plan every engine transcribes
```

## How it works

```text
probe_decode ─▶ separation ─▶ vad ─▶ asr ─▶ diff_sheet ─▶ sound_events ─▶ adjudication
                                                                              │
                                output ◀─ qc ◀─ cues ◀─ alignment ◀───────────┘
```

The arrows are the run order of `StageName::ALL`. A stage reads what the stages before it wrote to
the job's work directory: `diff_sheet` reads every engine's words from `asr`, `adjudication` reads
the diff sheet and the sound-event candidates, `alignment` reads the adjudicated text, and so on
down to `output`. Each module holds only a `mod.rs` with its header.

## Public surface

- One public module per stage, named as the stage is named on the command line: for the job runner
  in `crates/pipeline/`. No module holds items yet.

## Boundaries

- Depends on: nothing yet; the crate declares `media_io`, `inference`, `subtitle_formats` and
  `job_model` for these modules.
- Used by: nothing yet; `crates/pipeline/` declares the crate as a dependency.
- Rules: each module is named exactly as its stage's `StageName::as_str` name, and a stage's output
  is complete or absent, never partial (the crate header in `lib.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#stage-flow) — the stage flow and every stage in
  detail.
