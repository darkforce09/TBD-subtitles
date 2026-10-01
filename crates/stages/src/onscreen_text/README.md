# On-screen text

The visual stages of a video job. They keep observations separate from spoken cues, produce
[ASS](/documentation/glossary.md#ass) events on a 1920 by 1080 script canvas and, for the
localized video, replace the writing itself with English lettering.

## Contents

```text
crates/stages/src/onscreen_text/
├── detect/               sampled screening, bisected boundaries, keyframe crops and stills
├── event_buffer.rs       bounded ASS event storage and adjacent frame coalescing
├── furigana.rs           kana ruby folded into the kanji line it annotates
├── geometry.rs           checked homographies and robust fitting
├── glyphs.rs             portable vector outlines for perspective lettering
├── keyframe_requests.rs  keyframe grouping, Claude prompts, answer schema and checks, added writing
├── known_signs.rs        signs the library holds: their keyframes skipped, the sign's English kept
├── mod.rs                stage module exports
├── png.rs                synced PNG writes: crops, stills, masks, plates, patches and previews
├── read.rs               Japanese readings and compatible adjacent fragments
├── reference.rs          scene-validated reference wording
├── replace/              stroke masks, inpainted plates and English lettering for the localized video
├── review.rs             source-identity checks and owner corrections
├── tests/                recognition, translation, geometry, tracking and rendering checks
├── track.rs              sampled geometry checks against the keyframe quad
├── translate.rs          library signs first, Claude keyframes, local translation for the rest, joining
├── typeset.rs            confidence checks, safe masks and nearby fallbacks
├── unify.rs              occurrences of one sign joined into one continuous span
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
finds outside the listed regions becomes a flagged nearby occurrence. `translate_known` also takes
the signs the pipeline found in the sign library for some occurrences (by id): a keyframe whose
every occurrence is a known sign is not sent, and each known occurrence ends as it entered the
stage but for the sign's reading, English and confidence, with the `library` provenance backend,
even when its keyframe was sent for other writing. A retry generation
refreshes a request once and then resumes its cached result. Local readings with at least four
katakana characters plus kanji cap their local translation confidence at 0.84.

Reading and translation consolidate compatible adjacent occurrences without crossing the shot
changes the caller passes;
translated wording and confidence bands must agree, and a gap of up to one and a half of the
shortest frame counts as one missing frame, which the earlier sighting then covers. All observed
frames and crops survive. A kana-only line folds into the one kanji line it sits on as ruby: its
height is 0.18 to 0.55 of the line's, its bottom lies between half a line height above the line's
top and a third of a line height into it, its centre stays over the line, it is at most 1.6 times
as wide, for at least two thirds of the time it shares with the line, which is at least half its
own span. The line keeps its reading and
English, records the ruby box at its keyframe in `ruby` and the ruby's reading in its reason.

At the end of translation, and again after owner corrections in the review step, `unify` joins the
occurrences that show one sign: the same Japanese (or one reading containing the other, with
spaces ignored and small kana read as full-size), keyframe
boxes that overlap by 0.3 or hold each other's centre, and spans that overlap or pause at most
0.25 s without a cut. The detector's occurrence survives over writing Claude found (`-c` ids),
then the longer span; it takes the joined span, fills the time only the others covered with its
own keyframe box, and tiles its frames without a hole. A reviewed occurrence is never absorbed and
keeps the owner's timing and English. Furigana grouping runs again afterwards, so ruby only Claude
found folds into the joined line. A re-run of the review step repairs an existing job without
Claude calls. Review rejects missing or mismatched source fingerprints and reports orphaned
corrections. The desktop lets the owner remove those orphans; this folder never silently discards
saved edits.

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
- Rules: source videos remain read-only; no model conversion or full-video image extraction; an uncertain mask never covers foreground artwork; stages read no step document from the job folder, the caller passes it; every PNG a document names is written through `png.rs`, synced before the stage returns.

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — orchestration and resume.
- [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md) — behaviour and review.
- [Text detection](/crates/stages/src/onscreen_text/detect/) — the sampled scan in detail.
