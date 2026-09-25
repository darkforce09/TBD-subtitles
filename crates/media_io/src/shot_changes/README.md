# Shot-change scan

FFmpeg's `scdet` filter over a 480-pixel copy of the video: every scene change it reports, with
its time and score, for the cue stage to snap timing to.

## Contents

```text
crates/media_io/src/shot_changes/
├── mod.rs  `scan` runs FFmpeg (CPU or NVDEC decoding), `parse` reads the score and time pairs
└── tests/  unit tests for the log parsing and a generated black-to-white cut
```

## Boundaries

- Depends on: `child_process::Run` for FFmpeg; the `job_model::outputs` shot types.
- Used by: `tools/stack_spike/` (the shots item).
- Rules: every change scoring `REPORT_THRESHOLD` or more is kept with its score; choosing the
  score that counts as a cut belongs to the cue stage (`reads_the_cut_times_from_the_log`).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#8-shot-changes) — the scdet command.
- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md) — how cues use cuts.
