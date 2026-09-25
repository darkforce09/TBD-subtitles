**Status:** live

# Japanese on-screen text

Translated subtitles for Japanese text that appears in the picture — signs, letters, newspapers,
wanted posters, title cards — placed near the text, alongside the dialogue subtitles. Planned for
milestone M4; nothing is built yet.

## Where it lives

- Code (planned): an `onscreen_text` stage in `crates/stages/`, with OCR and vision backends in
  `crates/inference/`.
- Related: the [pipeline](/documentation/architecture/pipeline.md) (it runs after shot changes are
  known) and the ASS writer in `crates/subtitle_formats/`.

## Open question first

The owner asked for "the Japanese text that sometimes shows up" to be translated. Before building,
confirm what that covers: text **written on screen** (the design below), and whether Japanese
**speech or songs** that remain in the dub edits should be translated too (that would need Japanese
speech recognition plus translation, a separate design).

## Behaviour

Two sources, used in this order:

1. **Existing sign translations (source A).** When reference subtitles for the same animation
   exist — for Dressrosa, the One Pace `.ass` files kept in the media folder's
   `_reference_japanese_version_subs/` — their sign events (styles such as Captions, Title, Note)
   are already translated and positioned. The dub edit's timeline differs, so each sign is
   re-timed: the reference dialogue events are matched to our dialogue cues (same scenes, same lip
   movements, similar meaning) to build a piecewise time map, the sign is mapped through it, and a
   frame check confirms the text is visible at the new time.
2. **Recognition and translation (source B).** For videos without references, or signs the
   reference lacks:
   1. Sample frames at 2–4 fps, skipping frames that have not changed (perceptual hash), and
      always one per shot.
   2. Detect text regions (PP-OCRv5 through oar-ocr); read them (manga-ocr for vertical or
      stylised text, PP-OCR otherwise).
   3. Send low-confidence crops to a vision model (Qwen3.5-4B or PaddleOCR-VL through mistral.rs).
   4. Track each text across frames by position and content to get when it appears and
      disappears.
   5. Translate with the language model, given the surrounding dialogue and the series glossary.
   6. Write sign events in ASS, placed near the text's box (`\pos`), or at the top (`{\an8}`) when
      the text sits where dialogue subtitles go.

## Data

- Reference subtitles: any `.ass` next to or below the video that the owner points at.
- Output: the job's `.ass` gains a sign style; with signs present the job writes `.ass` instead of
  `.srt` (see the open question on formats in the [roadmap](/documentation/roadmap.md#open-questions)).

## Design

Sign subtitles use a smaller, distinct style from dialogue so the two never read as one another.

## Open work

- Milestone M4 in the [roadmap](/documentation/roadmap.md#m4--japanese-on-screen-text).

## Decisions

- Reuse existing human sign translations before machine OCR: they are better, and the One Pace
  reference files for Dressrosa already exist on disk.
