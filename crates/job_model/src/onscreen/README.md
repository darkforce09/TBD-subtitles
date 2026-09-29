# On-screen text contracts

Visible writing, tracked geometry, English translations and owner corrections shared by all layers.

## Contents

```text
crates/job_model/src/onscreen/
├── mod.rs       geometry, observations, translations, corrections and summary counts
├── settings.rs  new-job and saved-job defaults
└── tests/       correction validation and compatibility defaults
```

## How it works

`TextDocument` holds occurrences in source-pixel coordinates and presentation seconds. Crops stay
relative to the job folder. `TextCorrections` keeps owner edits separate from generated results.
`TextSettings::new_job` enables translation; missing settings in a saved job leave it disabled.

## Boundaries

- Depends on: `serde` and `std`.
- Used by: inference, stages, pipeline, subtitle formats and the desktop window.
- Rules: untranslated text carries no invented English; edits have finite valid timing and size.

## Related documentation

- [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md) — feature behavior.
