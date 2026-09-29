# On-screen text

The six visual stages of a video job. They keep observations separate from spoken cues and produce
[ASS](/documentation/glossary.md#ass) events on a 1920 by 1080 script canvas.

## Contents

```text
crates/stages/src/onscreen_text/
├── detect.rs        fixed-anchor detections and perspective-corrected crops
├── event_buffer.rs  bounded ASS event storage and adjacent frame coalescing
├── geometry.rs      checked homographies and robust fitting
├── glyphs.rs        portable vector outlines for perspective lettering
├── mod.rs           stage module exports
├── read.rs          Japanese readings, compatible fragments and furigana evidence
├── reference.rs     scene-validated reference wording
├── review.rs        source-identity checks and owner corrections
├── tests/           recognition, translation, geometry and rendering checks
├── track.rs         forward/backward optical flow and detection re-anchoring
├── translate.rs     local translations and verified-occurrence consolidation
├── typeset.rs       confidence checks, safe masks and nearby fallbacks
└── vision.rs        bounded parallel image verification through the Claude CLI
```

## How it works

Detection streams source frames with presentation timestamps. Fixed anchor crops, local signature
cells and mutually unique matches separate changed writing; perspective-corrected crops aid OCR
while surface checks use the original picture. Tracking checks flow in both directions and
rejects uncertain geometry. Local translation precedes up to four Claude image workers under the
shared call cap. A retry generation refreshes a request once and then resumes its cached result.
Local readings with at least four katakana characters plus kanji require image verification or
owner review; the guard caps their local translation confidence at 0.84.

Reading and translation consolidate compatible adjacent occurrences without crossing known cuts;
translated wording and confidence bands must agree. All source frames and crops survive. Ruby
becomes evidence only when its geometry fits and its translated words are covered by the parent.
Review rejects missing or mismatched source fingerprints and reports orphaned corrections. The
desktop lets the owner remove those orphans; this folder never silently discards saved edits.

Typesetting withholds unreviewed confidence below 0.85 or non-finite scores, retaining candidate
English and an explicit warning. Accepted wording uses safe replacement or a nearby label; unsafe
masks never cover the picture, and overcrowded labels remain unrendered and flagged.

The scan holds at most one million geometry observations and one hundred thousand occurrences.
Readings without Japanese script or with OCR confidence below 0.5 bypass local translation;
image verification or an explicit unresolved flag preserves them for review.
Exceeding either limit fails explicitly and asks for shorter inputs; it never truncates coverage.
Typesetting holds at most 128 MiB in each event buffer. Excessive per-occurrence vector lettering
uses a flagged nearby label; a combined output beyond the buffer limit fails explicitly.

## Boundaries

- Depends on: `job_model`, `media_io`, `inference`, `subtitle_formats` and pure Rust geometry and font libraries.
- Used by: `pipeline::tasks::onscreen`.
- Rules: source videos remain read-only; no model conversion or full-video image extraction; an uncertain mask never covers foreground artwork.

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — orchestration and resume.
- [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md) — behaviour and review.
