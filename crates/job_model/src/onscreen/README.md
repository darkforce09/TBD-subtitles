# On-screen text contracts

Visible writing, tracked geometry, English translations and owner corrections shared by all layers.

## Contents

```text
crates/job_model/src/onscreen/
├── localize.rs  replacement contracts: stroke masks, plates, patches and the localized video record
├── mod.rs       geometry, observations, translations, corrections and summary counts
├── settings.rs  new-job and saved-job defaults
├── verify.rs    the read-back check's result: replacements with final statuses and each reading
└── tests/       correction validation, compatibility defaults, replacement invariants, rkyv
```

## How it works

`TextDocument` holds occurrences in source-pixel coordinates and presentation seconds. Crops stay
relative to the job folder. `TextCorrections` keeps owner edits separate from generated results.
`TextSettings::new_job` enables translation; missing settings in a saved job leave it disabled.
`VerifiedReplacements` flattens its `ReplacementDocument` into the top level of
`visual/text_verify.json`, so the file also reads as a plain document, and adds `checks`: per
checked occurrence each sampled frame's `VerifyReading`.

## Boundaries

- Depends on: `serde` and `std`.
- Used by: inference, stages, pipeline, subtitle formats and the desktop window.
- Rules: untranslated text carries no invented English; edits have finite valid timing and size.

## Related documentation

- [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md) — feature behavior.
