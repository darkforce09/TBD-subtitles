# On-screen text contracts

Visible writing, tracked geometry, English translations and owner corrections shared by all layers.

## Contents

```text
crates/job_model/src/onscreen/
├── frames.rs    the `frames` rows: each frame's quad, follow score, shift, run-length mask and plate
├── library.rs   one approved sign of the library shared by episodes: English, style, patch, mask
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
patches letter the shifts its frames take besides its own. `LibrarySign` is one approved sign as
`library.redb` in the app's data folder keeps it: the English, its confidence and lettering style,
the patch and mask of its first plate, and the jobs that recorded it, the first being its origin.

## Boundaries

- Depends on: `serde`, `rkyv` and `std`; `sha2` for an occurrence's observation fingerprint.
- Used by: `crates/inference/` (the OCR's quads), `crates/stages/`, `crates/pipeline/`, the
  desktop window and `tools/visual_validation/`.
- Rules: untranslated text carries no invented English; edits have finite valid timing and size
  (`edits_reject_invalid_timing_and_sizes`,
  `edit_validation_rejects_nonfinite_and_out_of_range_fields` in `tests/contracts.rs`); a mask's
  runs decode back to the mask and stay inside its plate
  (`a_mask_encodes_into_runs_and_decodes_back`, `runs_outside_the_mask_are_clipped` in
  `tests/frames.rs`); the check's document also reads as a plain replacement document
  (`the_file_reads_back_whole_and_as_a_plain_replacement_document` in `tests/verify.rs`).

## Related documentation

- [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md) — feature behavior.
