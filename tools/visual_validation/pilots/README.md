# Visual pilot annotations

Independent observations of the owner's Dressrosa clips, used to check recognition coverage.

## Contents

```text
tools/visual_validation/pilots/
├── annotation_notes.json  source offsets, clipped fragments and annotation limitations
├── dressrosa_11.json      role and name card
├── dressrosa_16.json      board in front and perspective views
├── dressrosa_39.json      fading title and one-frame remnants
├── example_board.json    supplied board still held for one second
├── example_name.json     supplied name-card still held for one second
└── example_title.json    supplied title-card still held for one second
```

## How it works

Extract the source clips at the offsets in `annotation_notes.json`: 11 seconds from episode 11,
12 seconds from episode 16 and 10 seconds from episode 39. These annotations come from examining
source frames independently of OCR. Furigana belongs to its base line. The notes explicitly retain
clipped fragments whose complete wording is unreadable; they are part of coverage review.

Timing uses native 24 fps, with exclusive end times. The manually marked quadrilaterals establish
overlap for flagged nearby translations. They cannot establish two-pixel tracking accuracy.

The three supplied Japanese stills are also held for one second at 24 fps with silent English
audio for the production harness. An odd image dimension receives one black padding pixel to
allow H.264 encoding. Their main text and approximate bounding boxes are annotated independently
from the source pictures; furigana belongs to its base line. These static inputs check the full
recognition, translation and ASS path, while the episode clips check actual motion and cuts.

## Boundaries

- Depends on: owner-provided source videos, which remain outside the repository.
- Used by: `visual_validation evaluate` and visual acceptance review.
- Rules: passing these annotated excerpts does not prove full-episode recognition coverage.

## Related documentation

- [Visual validation](/tools/visual_validation/README.md) — commands and acceptance checks.
