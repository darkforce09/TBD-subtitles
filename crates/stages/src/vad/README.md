# Voice activity stage

[Voice activity](/documentation/glossary.md#vad) found in a 16 kHz stem with earshot, the pure-Rust
detector, turned into padded and
merged speech regions and the chunk plan every speech engine transcribes.

## Contents

```text
crates/stages/src/vad/
├── chunk_plan.rs  regions to chunks: 20 to 60 s, cut in silences of 0.35 s, always at 3 s pauses
├── mod.rs         earshot scores per 16 ms frame of a stem, the settings, and `plan`
├── regions.rs     scores to regions: threshold 0.5, 200 ms padding, gaps under 300 ms merged
└── tests/         unit tests for the regions and the chunk cutting rules
```

## How it works

```text
stem .f32 ──score_file──▶ one earshot score per 256 samples ──regions::from_scores──▶ regions
regions + scores ──chunk_plan::cut──▶ chunks ──▶ SpeechPlan { frame_s, threshold, regions, chunks }
```

A chunk begins at a region's start and ends at a region's end, so its cuts lie in silences. It
grows over further regions until it is at least 20 s long and the next silence is 0.35 s or more;
a pause of 3 s or more always ends it, and it never passes 60 s. A single region longer than 60 s
is split at its quietest frame between 30 and 60 s from its start: the one place a cut may fall
inside speech.

## Boundaries

- Depends on: `earshot` (the detector), `media_io::pcm_stream::F32FileReader` (the stem),
  `job_model::outputs::{SpeechPlan, TimeSpan}`.
- Used by: `crates/pipeline/src/tasks/speech.rs` (the voice-activity step, in the job runner) and
  `tools/stack_spike/` (the vad item).
- Rules:
  - chunks are ordered, disjoint and never longer than 60 s (`chunks_are_ordered_and_disjoint`,
    `no_chunk_passes_the_maximum` in `tests/chunk_plan.rs`);
  - a long pause always ends a chunk (`a_long_pause_always_ends_a_chunk`);
  - regions stay inside the track after padding
    (`close_regions_merge_and_padding_stays_inside_the_track` in `tests/regions.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#3-voice-activity-and-chunk-plan) — the
  voice-activity settings and the chunk plan.
- [Rust ML stack](/documentation/research/rust_ml_stack.md#2-voice-activity-detection) — earshot
  and the other detectors.
