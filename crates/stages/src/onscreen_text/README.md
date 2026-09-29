# On-screen text

The six visual stages of a video job. They keep observations separate from spoken cues and produce
[ASS](/documentation/glossary.md#ass) events on a 1920 by 1080 script canvas.

## Contents

```text
crates/stages/src/onscreen_text/
├── detect/               sampled screening, bisected boundaries, keyframe crops and stills
├── event_buffer.rs       bounded ASS event storage and adjacent frame coalescing
├── geometry.rs           checked homographies and robust fitting
├── glyphs.rs             portable vector outlines for perspective lettering
├── keyframe_requests.rs  keyframe grouping, Claude prompts, answer schema and checks, added writing
├── mod.rs                stage module exports
├── read.rs               Japanese readings, compatible fragments and furigana evidence
├── reference.rs          scene-validated reference wording
├── review.rs             source-identity checks and owner corrections
├── tests/                recognition, translation, geometry, tracking and rendering checks
├── track.rs              sampled geometry checks against the keyframe quad
├── translate.rs          Claude keyframe reading first, local translation for the rest, consolidation
├── typeset.rs            confidence checks, safe masks and nearby fallbacks
└── vision.rs             bounded parallel keyframe requests through the Claude CLI
```

## How it works

Detection streams a proxy copy of every frame with packet presentation timestamps, screens the
samples (every `round(fps / 2)`-th frame plus both frames around each cut) with the local
detector, and bisects the frames between two samples to the exact frame where writing appears or
vanishes. Fixed anchor signatures, local signature cells and mutually unique matches separate
changed writing; each occurrence keeps one frame per sample, a keyframe still nearest its midpoint
and a perspective-corrected crop from that still, whose surface colour the original picture
supplies. Tracking checks that the sampled quads stay within tolerance of the keyframe quad; a
moving surface gets nearby placement. When the Claude fallback is on, each keyframe still is sent
once, whole frame plus region crops, through up to `llm_processes` workers under the shared call
cap; the local model then opens only for occurrences Claude did not answer, and writing Claude
finds outside the listed regions becomes a flagged nearby occurrence. A retry generation
refreshes a request once and then resumes its cached result. Local readings with at least four
katakana characters plus kanji cap their local translation confidence at 0.84.

Reading and translation consolidate compatible adjacent occurrences without crossing known cuts;
translated wording and confidence bands must agree. All observed frames and crops survive. Ruby
becomes evidence only when its geometry fits and its translated words are covered by the parent.
Review rejects missing or mismatched source fingerprints and reports orphaned corrections. The
desktop lets the owner remove those orphans; this folder never silently discards saved edits.

Typesetting withholds unreviewed confidence below 0.85 or non-finite scores, retaining candidate
English and an explicit warning. Accepted wording uses safe replacement or a nearby label; unsafe
masks never cover the picture, and overcrowded labels remain unrendered and flagged.

The scan holds at most one million geometry observations and one hundred thousand occurrences,
and fails when the decoded frame count differs from the packet table. Readings without Japanese
script or with OCR confidence below 0.5 bypass local translation; Claude's reading or an explicit
unresolved flag preserves them for review. Exceeding a limit fails explicitly and asks for shorter
inputs; it never truncates coverage. Typesetting holds at most 128 MiB in each event buffer.
Excessive per-occurrence vector lettering uses a flagged nearby label; a combined output beyond the
buffer limit fails explicitly.

## Boundaries

- Depends on: `job_model`, `media_io`, `inference`, `subtitle_formats` and pure Rust geometry and font libraries.
- Used by: `pipeline::tasks::onscreen`.
- Rules: source videos remain read-only; no model conversion or full-video image extraction; an uncertain mask never covers foreground artwork.

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — orchestration and resume.
- [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md) — behaviour and review.
- [Text detection](/crates/stages/src/onscreen_text/detect/) — the sampled scan in detail.
