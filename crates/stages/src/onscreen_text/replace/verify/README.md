# Read-back check

The step after lettering composition. For every baked occurrence it rebuilds a few frames as the
localized video will show them and has a local OCR read them back; an occurrence stays in the
video only when no frame still shows Japanese where the writing was and every frame reads back as
its English.

## Contents

```text
crates/stages/src/onscreen_text/replace/verify/
├── area.rs     the lettering placed on a frame's plate, the region read, and where a line counts
├── mod.rs      `ReadBack`, `LocalOcr`, `verify`: samples, the finished picture, the readings
├── samples.rs  which frames of an occurrence are read
├── verdict.rs  a frame's and an occurrence's verdict from the readings: kana, similarity
└── tests/      frame choice, geometry, verdicts and whole runs over scripted frames and readers
```

## How it works

`samples::sample_frames` takes an occurrence's first, middle and last frame, the frames either
side of where another baked occurrence over the same place (intersection over union of 0.3 or
more) starts and ends, then its plates' first frames, spread evenly: distinct, at most 8.

For each sampled frame, in frame order, `area::sample_area` places the keyframe lettering quad
(the `lettering_quad`, else the tracked quad) and the furigana on the plate covering the frame, at
the frame's shift in the request's `Motion` (from the `frames` rows), as composition lettered them, and grows them by 0.75 of a line into a region on even pixels. The
`RegionSource` decodes that region of the source frame; every patch the localized video blends at
that frame (`localize::patches::Schedule`, the patch of each frame's shift) is blended over it by `localize::still`, which uses the
render's own Y′CbCr blend; the region is enlarged so a line is 48 pixels tall, within 8 million
pixels.

`ReadBack` finds the lines and reads each; `LocalOcr` is PP-OCRv5's server detector and its
recognizer alone (`OcrReader::read_primary`), which reads Japanese and Latin alike. Lines are
read in rows, top to bottom and left to right. A line counts as the English when half its box
lies over the lettering grown by a quarter of a line; as the writing's own place when half lies
over the lettering grown a quarter of a line at the sides and 0.75 of a line above (its own
furigana), or when it is at least 0.6 of a line tall and counts as the English (its own line,
missed by the erase). Small writing below the lettering is the next line's furigana and does not
count.

`verdict::judge` fails a frame when a reading in the writing's place holds 2 or more hiragana,
katakana or kanji at a confidence of 0.5 or more, or when the letters and digits read as the
English (lowercased, accents and fullwidth forms folded) are less than 0.6 similar to the
English: 1 minus the edit distance over the longer length. `verdict::verdict` turns an
occurrence with a failed frame to `Fallback`: “The finished picture still shows Japanese” when
any frame showed Japanese, else “The English does not read back cleanly”.

`verify` returns a `Verified`: the `VerifiedReplacements` (the composed document with the final
statuses, and per checked occurrence the frames read and whether all passed) and every frame's
`VerifyReading` with its occurrence id, which the task stores as `readings` rows. An observer sees each sample's lines and
the enlarged picture read, which the `visual_validation` tool's `verify-probe` prints and saves.

## Boundaries

- Depends on: `job_model::onscreen` (replacement documents, readings), `inference::ocr`
  (PP-OCRv5), the sibling `compose` placement helpers and `detect::crop`, `localize::patches`,
  `localize::still`, `localize::colour::Conversion` and `localize::motion::Motion`, and `image`.
- Used by: `pipeline::tasks::verify`, in a `tbd-subtitles` ONNX Runtime worker under the GPU
  lock; the `visual_validation` tool's `verify-probe`.
- Rules: only baked occurrences are checked, and only a failed check changes one; files and the
  source video are only read; one region and its patches are held at a time.

## Related documentation

- [Video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md#the-read-back-check-text_verify)
  — the step, its thresholds and the numbers behind them.
- [Stack and pipeline decisions](/documentation/decisions/stack_and_pipeline.md) — why the gate
  is a local OCR.
