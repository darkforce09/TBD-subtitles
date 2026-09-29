# Text review rendering

The Check Text list, editor and original/rendered comparison.

## Contents

```text
apps/tbd_subtitles/src/text_review/ui/
├── mod.rs      rendering exports
├── preview.rs  bounded textures and playback controls
└── review.rs   occurrence list and correction editor
```

## How it works

The view borrows a session and decoded comparison, then returns events for application actions. Preview shows pixels from the actual ASS renderer.

## Boundaries

- Depends on: text review models and core UI styling.
- Used by: application feature views.
- Rules: rendering does not start processes or write corrections.

## Related documentation

- [Check Text](/apps/tbd_subtitles/src/text_review/) — the feature.
