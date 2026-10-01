# On-screen text contracts

Visible writing, tracked geometry, English translations and owner corrections shared by all layers.

## Contents

```text
crates/job_model/src/onscreen/
├── frames.rs    the `frames` rows: each frame's quad, follow score, shift, run-length mask and plate
├── localize.rs  replacement contracts: stroke masks, plates, patches and the localized video record
├── mod.rs       geometry, observations, translations, corrections and summary counts
├── settings.rs  new-job and saved-job defaults
├── verify.rs    the read-back check's result: final statuses, verdicts, the `readings` row type
└── tests/       corrections, defaults, replacement invariants, run-length masks, rkyv
```

## How it works

`TextDocument` holds occurrences in source-pixel coordinates and presentation seconds. Crops stay
relative to the job folder. `TextCorrections` keeps owner edits separate from generated results.
`TextSettings::new_job` enables translation; missing settings in a saved job leave it disabled.
`VerifiedReplacements` flattens its `ReplacementDocument` into the top level of its JSON, so it also
reads as a plain document, and adds `checks`: per checked occurrence the frames read and whether
all passed; each sampled frame's `VerifyReading` is a row of the `readings` table, and `telling`
picks the one that says most. `FrameRecord` is one row of the `frames` table: where the writing
sits in one frame (its quad, follow score, shift and scale), its erase mask as `RleRun`s relative
to its plate (`encode_mask`, `decode_mask`, `mask_area`) and its plate. A plate's `shifted`
patches letter the shifts its frames take besides its own.

## Boundaries

- Depends on: `serde` and `std`.
- Used by: inference, stages, pipeline, subtitle formats and the desktop window.
- Rules: untranslated text carries no invented English; edits have finite valid timing and size.

## Related documentation

- [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md) — feature behavior.
