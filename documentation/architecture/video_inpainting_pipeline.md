**Status:** live

# Video inpainting and in-place text replacement

How the job replaces visible Japanese writing inside the picture: it erases each translated
occurrence's strokes, fills the background behind them with an inpainting model, letters the
English in the original writing's place, colour and weight, and re-encodes the whole video as a
[localized video](/documentation/glossary.md#localized-video) beside the source. Writing that
cannot be replaced cleanly keeps its English in the localized video's own subtitle file. The
source video is only read. What the owner sees of it is in
[Japanese on-screen text](/documentation/features/japanese_onscreen_text.md#replacement-in-the-video).

## Why the picture is edited

An ASS subtitle file can draw shapes and text over the video but cannot remove a pixel. Over
artwork (a scroll, a name card, a textured wall) the Japanese stays visible under any English an
ASS file places on it, so the ASS typesetter puts most such writing beside the Japanese instead
(`safe_surface` in the [typesetter](/crates/stages/src/onscreen_text/typeset.rs)). Replacing the
writing where it stands needs the picture itself: the strokes erased at the pixel level, the
background under them reconstructed, and the English drawn into the frames, as Google Translate
does for photographs.

## Flow

```text
text_review ─▶ text_mask ─▶ text_inpaint ─▶ text_compose ─▶ text_verify ─▶ text_typeset ─▶ qc ─▶ output ─▶ localized_video
  reviewed      stroke masks  LaMa fills     English          a local OCR   ASS events      <video>.ass        <video>.localized.mkv
  English       source plates the plates     patches          reads each    for             <video>.localized.ass
                                                              one back      <video>.ass     (dialogue and sound cues)
```

| Step | Runs in | Reads | Writes |
|---|---|---|---|
| `text_mask` | worker, `tbd-subtitles` (CPU; FFmpeg region crops) | `outputs/probe_decode`, `outputs/text_review`, the source video | `outputs/text_mask`, `visual/masks/` |
| `text_inpaint` | worker, `tbd-subtitles` (ONNX Runtime, GPU lock) | `outputs/text_mask` and its PNGs | `outputs/text_inpaint`, `visual/plates/` |
| `text_compose` | worker, `tbd-subtitles` (CPU) | `outputs/text_review`, `outputs/text_inpaint`, the `latin-fonts` model | `outputs/text_compose`, `visual/patches/` |
| `text_verify` | worker, `tbd-subtitles` (ONNX Runtime, GPU lock; FFmpeg region crops) | `outputs/probe_decode`, `outputs/text_review`, `outputs/text_compose` and its patches, the source video | `outputs/text_verify` |
| `text_typeset` | worker, `tbd-subtitles` (CPU) | `outputs/text_review` | `outputs/text_typeset`, `outputs/text_typeset/ass` |
| `output` | job runner | `outputs/cues`, `outputs/text_typeset` and its ASS events, `outputs/text_verify` | `<video>.ass` (or the chosen format), `<video>.localized.ass`, `outputs/output` |
| `localized_video` | worker, `tbd-subtitles` (FFmpeg decoder and encoder, GPU lock) | `outputs/probe_decode`, `outputs/text_verify` and its patches, `outputs/output`, the source video | `<video>.localized.mkv`, `outputs/localized_video` |

The three replacement steps and the read-back check pass one `ReplacementDocument`
([contract](/crates/job_model/src/onscreen/localize.rs)) from step to step, each adding to it:
one `ReplacedText` per candidate occurrence with its frame span, its status (`pending`, `baked`
or `fallback` with a reason), its measured `LetteringStyle`, its container, its
[plates](/documentation/glossary.md#plate) and, for a loose Claude box, the `lettering_quad` the
English goes in. Frame numbers index the video's presentation
timeline from zero; paths are relative to the job directory. A job with the localized video off
writes an empty document and an empty event file at each step, loads no model and starts no
encoder.

## Stroke masks (`text_mask`)

Code: [mask](/crates/stages/src/onscreen_text/replace/mask/), with region crops decoded by
[`FfmpegRegions`](/crates/stages/src/onscreen_text/replace/source.rs).

- **Candidates:** an occurrence with non-empty English that is reviewed or has a confidence of at
  least 0.85, with observed frames and a keyframe. Its span is every frame whose start lies in
  its `[start_s, end_s)`. An occurrence the owner set to Nearby in Check Text, or a span with no
  frame, falls back at once. Nearby placement chosen by an earlier step for the ASS file (moving
  writing, writing only Claude found) does not stop an attempt.
- **Regions, not frames:** only the keyframe's plate is decoded for segmentation, then the
  span's plate region once, each at full resolution through a region stream that converts each
  frame to RGB and crops it. The analysis window is the bounds of the keyframe quad and of the
  furigana folded into the line (`ruby`) grown by 4 pixels; the plate is those bounds grown by
  `max(32, 0.75 × the quad's shorter side)`, the context inpainting sees. The English is still
  lettered in the line's own quad. A box Claude found on a keyframe (an id ending in `-c` and a
  number) is loose: its window and plate grow by a further 0.35 of its height, and when the
  padded window does not separate, the unpadded one is tried.
- **Separating strokes:** the window's pixels are partitioned in CIE Lab with k = 2 to 5
  (k-means, at most 20 iterations, centres fitted on at most 65,536 pixels). Each partition is
  read first as lettering over the background the window's 3-pixel border ring shows: colours
  that dominate the ring, or are no rarer there than inside, are background; a core ink colour
  sits at least 2.0 spreads from every background colour; a fill wrapped by an outline counts even
  on a noisy translucent panel, and only the outline band around the fill is ink, so line art of
  the outline's colour stays. When no partition reads as lettering, it is read as writing printed
  on a panel or sign that fills the box. When neither passes and the box is the detector's, it is
  read as outlined lettering whose fill and outline also run along the box's edges (a card's
  ruled frame the box grazes, a picture of the fill's colour at its side): a fill and outline
  wrapping each other are the ink whatever their share of the ring, when both are flat (spread at
  most 15), at least 1.0 spread from every other ring colour, and at least half the fill's pixels
  have three of their four neighbours in the fill. Strokes the box clips are followed across the
  plate by at most `max(8, half a line)` pixels; ink inside a furigana box is ink even where the
  window cuts it, and counts as covered, never cut; without furigana boxes, furigana above
  outlined lines (on a panel too) are added; writing the box cuts off falls back. A fill piece
  whose edge is less than 80 % ringed by its outline is picture in the fill's colour (a light
  collar beside white lettering); a piece spanning more than 2.5 line heights one way and 0.8 the
  other, filling less than 35 % of its bounds and enclosing half the other ink, is a frame; for a
  loose box, a piece with less than a third of its pixels inside the box is picture beside it;
  none of them is ink. A partition passes when its
  ink covers 1 % to 55 % of the line's quad; among the nearly cleanest passing partitions the one
  covering most of the quad and its furigana wins. The full rules:
  [mask stage](/crates/stages/src/onscreen_text/replace/mask/).
- **Completion:** a pixel is ink-coloured when its nearest cluster is ink and it lies within
  ΔE 12 of the measured fill or outline. Within half a line around the window, 8-connected pieces
  of ink-coloured pixels that touch the ink, end inside that region and hold at most a quarter of
  the ink's area and a stroke's width (outline included) times a line height join it; pieces
  running on past the region are picture. Ink-coloured pieces apart
  from the ink that stay inside the region are strays: more of them inside the quad and furigana
  than 8 % of the erase mask's area falls back. For a loose box the lettering area is refitted
  to the bounds of the ink inside the window and stored as `lettering_quad`; ink covering less
  than half of the box falls back. The [stroke mask](/documentation/glossary.md#stroke-mask) is
  the completed ink dilated by `max(2, 0.06 × line height)` pixels, 255 where pixels are erased;
  panels and pictures are never marked.
- **Style:** fill and outline are the core ink colours with the most ink, at least 2.0 spreads
  apart, the outline nearer the outside. The fill colour is the median of the eroded fill; stroke
  thickness is twice the fill area over its perimeter, outline thickness the outline area over
  it; an outline is soft when the band outside it departs from the background colour clearly more
  often than the border ring does.
- **Moving writing:** writing whose sampled quads all sit within half a pixel of the keyframe quad
  is static. Other writing is followed frame by frame: the keyframe window in grey, at five scales
  from 0.9 to 1.1, is matched by zero-mean normalized cross-correlation around the position
  interpolated from the sampled quads, within `0.5 × the shorter side + 16` pixels, coarse on
  block averages and then at full size around the best two matches. One frame scoring under 0.8
  loses the occurrence, and so does a swept region over a quarter of the frame.
- **Background runs:** a frame joins the current run while its placement is unchanged and the
  plate pixels outside the mask differ from the run's first frame by a mean under 3 and a 99th
  percentile under 24 per channel value. Each run is one plate, whose source is its first frame;
  more than 2,000 plates falls back.
- **Files:** `visual/masks/<occurrence>/mask.png` for the keyframe placement, `mask-<n>.png` for
  each other placement and `source-<n>.png` per plate. The folder is emptied at the start of the
  step, and an occurrence that falls back loses its folder.

## Inpainting (`text_inpaint`)

Code: [inpaint](/crates/stages/src/onscreen_text/replace/inpaint/) over
[LaMa](/crates/inference/src/onnx/lama/).

- **Model:** LaMa, `lama_fp32.onnx` from `Carve/LaMa-ONNX` (Apache-2.0, 208 MB, model folder
  `lama-inpaint`), a fixed 512 × 512 graph run through ONNX Runtime on CUDA. No pure-Rust
  inpainting network exists, so it runs under the native-runtime rule of
  [law 4](/CLAUDE.md#1-project-laws), in a `tbd-subtitles` worker under the GPU lock. The graph
  blanks the masked pixels itself; pixels outside the mask come back exactly as they went in.
- **Fitting a plate to 512:** a plate that fits sits at the square's top-left; one whose longest
  side is at most 1,024 is scaled so that side is 512; a larger one is halved and covered by
  512-pixel tiles overlapping by at least 128, blended linearly across each overlap, and a tile
  without a masked pixel costs no call. The square past the picture mirrors it, and the mask
  mirrors with it. A scaled mask sets a pixel when any pixel it covers is set, then grows by one.
- **Blend back:** the filled picture is scaled back with a triangle filter and laid over the
  source fully on masked pixels and by the masked share of each 3 × 3 box elsewhere, so pixels
  more than one pixel from the mask keep their source bytes.
- **Residue check:** on every filled plate of an occurrence with a measured style, a masked pixel
  still looks like the lettering when it lies within ΔE 12 of the fill colour and more than
  ΔE 20 from the median of the fill around it (a 9 × 9 grid over a quarter of a line on each
  side), in a blob that survives an opening by an eighth of the stroke thickness, so thin joints
  and cracks the fill continues do not count. Over 3 % of the mask's pixels, the plate is filled
  once more with the mask grown by a tenth of a line, written as
  `visual/plates/<occurrence>/<n>-mask.png` and recorded as the plate's mask so the patch covers
  it. The wider fill stands even when it is still over 3 %: whether the finished replacement is
  clean is the [read-back check](#the-read-back-check-text_verify)'s call, since a pixel rule on
  one plate cannot tell leftover strokes from lettering-coloured background (the owner saw it
  reject Dressrosa 28's 幹部塔 and スクラップ場 at 12:55 while the same signs baked cleanly at
  2:38). The check reads only plate pixels on the CPU and is deterministic.
- **Files:** `visual/plates/<occurrence>/<n>.png`, written under a temporary name and renamed. A
  plate whose source and mask files match an earlier plate's byte for byte reuses its result,
  from a cache of at most 64 plates and 256 MiB. One plate is decoded at a time; a plate with an
  empty mask is its source, with no model call.

## Lettering (`text_compose`)

Code: [compose](/crates/stages/src/onscreen_text/replace/compose/).

- **Font:** Noto Sans, the variable font with width and weight axes (`NotoSans.ttf`, OFL, model
  folder `latin-fonts`), read with `ttf-parser` and rasterized with `tiny-skia`, both pure Rust.
  A character the font cannot draw falls back.
- **Lettering area:** the occurrence's quad at its keyframe, or for a loose Claude box the
  `lettering_quad` the mask step refitted to the ink.
- **One replacement per sign:** of the occurrences about to be lettered, two on screen together
  whose lettering areas overlap by an intersection over union of 0.3, or where one covers half of
  the other, are one sign: the detector's is kept before Claude's, then the longer on screen,
  then the larger, then the earlier in the document, and the other falls back. No two patches
  are ever drawn over one piece of writing.
- **Containers:** occurrences on screen together whose keyframe quads, each grown by one line
  height, touch form one container, as a card's role line and name do; members share one scale
  factor, so their sizes keep the source's ratio.
- **Fitting:** the target cap height is 0.7 of the original line height. The English wraps at
  spaces (one word per line in an area more than 1.6 times taller than wide) and must fit 92 % of
  the rectified quad's width and 90 % of its height at 1.15 em line spacing; at each size the
  width axis tries 100, 87.5 and 75 before the size shrinks by 4 %. A cap height under 14 pixels
  per 1,080 frame lines falls back as too small to read. The weight follows stroke thickness over
  line height: 400, 600, 700 or 900.
- **Colour:** the measured fill, and the measured outline at no less than 1 pixel. With no
  outline, a fill whose WCAG contrast with the plate under it is below 3:1 gets a 2-pixel black or
  white outline, whichever contrasts more.
- **Drawing:** the lettering is drawn at twice the source resolution (at most 4,096 pixels a
  side), outline under fill, blurred when soft, then warped onto each plate through the inverse
  homography of the keyframe quad as that plate's placement moved and scaled it, sampled
  bilinearly.
- **Patches:** each plate's [patch](/documentation/glossary.md#patch) is an RGBA PNG,
  `visual/patches/<occurrence>/<n>.png`, whose colour is the lettering over the inpainted plate
  and whose alpha is the larger of the feathered erase mask and the lettering's coverage;
  `preview.png` shows the keyframe plate's patch over its original pixels for Check Text. Once
  every plate has its patch, the occurrence is `baked`.

## The read-back check (`text_verify`)

Code: [verify](/crates/stages/src/onscreen_text/replace/verify/), the task
[`tasks/verify.rs`](/crates/pipeline/src/tasks/verify.rs), and the composite
[`localize::still`](/crates/stages/src/localize/still.rs).

Nothing before this step looks at the finished picture: the mask, fill and lettering rules each
judge one keyframe's pixels. This step approves each baked occurrence from the frames as the
localized video will show them, with a local OCR, so it costs no API call however many episodes
run.

- **Frames:** per baked occurrence its first, middle and last frame; for an occurrence whose
  keyframe area overlaps another baked one's by an intersection over union of 0.3 or more, the
  frames either side of where the other starts and ends; then the first frame of each plate,
  spread evenly when they do not all fit. Distinct frames, at most 8.
- **The finished picture:** the lettering area (the `lettering_quad`, else the tracked quad at
  the keyframe) and its furigana, placed on the plate covering the frame as composition placed
  it, grown by 0.75 of a line (the measured line height) into a region on even pixels. FFmpeg
  decodes that region of the source frame at full resolution; every patch the localized video
  blends at that frame is blended over it with the render's own Y′CbCr blend in 8-bit 4:2:0 and
  converted back to RGB, and the region is enlarged so a line is 48 pixels tall (at most
  8 million pixels).
- **Reading:** PP-OCRv5's server detector finds the lines (box score 0.5) and its recognizer
  alone reads each, Japanese and Latin alike, never the Japanese-only manga-ocr. Lines are read
  in rows, top to bottom and left to right. A line counts as the English when half its box lies
  over the lettering grown by a quarter of a line, and as the original writing's place when half
  lies over the lettering grown a quarter of a line at the sides and 0.75 of a line above, where
  its own furigana sits, and not below, where the next line's does.
- **Verdict per frame:** a reading in the writing's place with 2 or more hiragana, katakana or
  kanji at a confidence of 0.5 or more means Japanese is left. The letters and digits read as the
  English (lowercased, accents folded, everything else dropped) are compared with the English
  the same way: similarity is 1 minus the edit distance from the English to its best-matching
  stretch of the reading over the English's length, so a neighbouring line's English read in the
  same area costs nothing; an English found a second time outside that stretch scores 0. Below
  0.6 the English does not read back (a doubled, garbled or missing lettering).
- **Verdict per occurrence:** it stays baked only when every frame passes; otherwise it falls
  back with “The finished picture still shows Japanese” when any frame showed Japanese, else
  “The English does not read back cleanly”.
- **Output:** `outputs/text_verify` is the replacement document with the final statuses at
  its top level (so its JSON also reads as a plain `ReplacementDocument`) and `checks`: per checked
  occurrence each frame's Japanese found, English read, similarity and pass. The localized video,
  the output step, the report and Check Text read it; the window shows
  `outputs/text_compose` while the step has not run. A document with nothing baked passes
  through without loading a model.
- **Measured thresholds:** on Dressrosa 28 (25 baked, 128 frames read in 39 s) every frame that
  read back the English scored 0.8 or more and the one lettering OCR could not read scored 0, so
  0.6 sits in an empty gap; every Japanese reading that counted (leftover furigana over 幹部塔,
  スクラップ場, 旧王台地 and ピカ像) was 0.7 or more confident, while the stray glyphs the detector
  finds in textures and outlines were one character or 0.6 and below. Five of the 25 fall back.
  Rerun end to end with the residue rule no longer rejecting, composition bakes 28 (スクラップ場
  at 12:55 and 正義 join) and the step approves 22 in 48 s (150 frames, 1.5 GB RAM, 1.8 GB VRAM);
  幹部塔 at 12:55 falls back again, now because its furigana stays readable. Dressrosa 11's 15
  baked replacements, the Rebecca name card and the 海 wall among them, all pass (70 frames,
  22 s, all 0.9 or more). The `visual_validation` tool's `verify-probe` prints
  every frame's lines, readings, confidences and verdict for a finished job.

## The localized subtitle file

`text_typeset` stores one set of ASS events, `outputs/text_typeset/ass`, with every displayable
occurrence; it goes into `<video>.ass` alone. `<video>.localized.ass` holds the dialogue and sound cues and no
on-screen text: the English is in the picture, and writing that could not be replaced stays
Japanese there, its reason in `outputs/text_verify`, the report and Check Text (`Not
replaced in the video: <reason>`). The output step reads `outputs/text_verify` and
`outputs/text_typeset`, turns each sampled frame of every baked occurrence into a rectangle
on the ASS canvas grown by 12 pixels, and moves a cue whose bottom box meets one while both are on
screen to the top with `{\an8}`
([subtitle style rules](/documentation/architecture/subtitle_style_rules.md#on-screen-text-and-the-localized-video)).
It backs up a different existing file into the job's `backup/` folder and records the file as
`localized` in `outputs/output`.

## The localized video (`localized_video`)

Code: [localize](/crates/stages/src/localize/), the task
[`tasks/localized.rs`](/crates/pipeline/src/tasks/localized.rs), and FFmpeg's
[encode](/crates/media_io/src/encode/) and [native frames](/crates/media_io/src/video_frames/).

- **Decode:** one FFmpeg decoder streams every frame at its native size, 10-bit 4:2:0 for a 10-bit
  4:2:0 source and 8-bit `yuv420p` otherwise. The decoded count must equal the timeline exactly.
- **Blend:** a schedule starts each baked plate's patch at its first frame and ends it after its
  last; overlapping patches stack in document order. Patches are converted once into Y′CbCr in the
  stream's tagged matrix and range (BT.709 from 720 lines when untagged, BT.601 below) and kept
  until their span ends, within 512 MiB. Luma blends per pixel and chroma per 2 × 2 block by its
  summed alpha; a clear pixel keeps its bytes. A frame with no active patch passes through as
  decoded.
- **Encode:** raw frames go through a pipe into a second FFmpeg that muxes Matroska: the new video,
  every audio stream of the source copied, its chapters and metadata, and no subtitle or data
  stream (`-sn -dn`). The encoder is `hevc_nvenc` (`-preset p6 -tune hq -rc vbr -cq 19`, `main` or
  `main10`) when FFmpeg lists it and a one-frame test encode runs, else libx264 (`-preset slow
  -crf 16`). The peak rate is capped at 1.25 times the source video's bit rate for NVENC and 1.5
  times for libx264 (`-maxrate`, `-bufsize` twice that); the rate is the stream's own, else the
  container's, else, for a probe written before the field existed, the file's size over the
  timeline's length. The frame rate is the stream's fraction, the first frame's time its offset,
  and the colour tags the probe knows are copied.
- **Guards:** a stream without a frame rate, and a timeline whose frames stray from their
  constant-rate positions by more than 1 % of a frame plus a millisecond (a variable frame rate),
  fail the step before encoding, since raw frames on a pipe carry one rate. The file is written to
  `<video>.localized.mkv.part` and renamed when whole; a failed encode removes the part file. A
  `<video>.localized.mkv` already there is replaced only when this job's record names it as its
  own; any other file fails the step with a message to move it away. A video itself named
  `.localized.mkv` is refused.
- **Record:** `outputs/localized_video` holds the path, the encoder, the frames written and
  the occurrences replaced. With the setting turned off it holds no path but keeps the earlier
  path as `earlier`, so turning it on again may overwrite the job's own file. The step's output
  counts as valid only while the recorded file exists; the output step's only while its
  `<video>.localized.ass` does.

## Bounds

- Memory: the mask step holds the current run's first frame and the frame being examined; the
  inpaint step one plate and a bounded cache; the compose step one plate's images; the encode one
  frame and the active patches, within 512 MiB. Masks, plates and patches are PNGs in the work
  directory.
- Time: each replacement step and the localized video have a six-hour step deadline; the encoder a
  24-hour one. The localized video reports its progress every 240 frames.
- Resume: `text_inpaint`, `text_compose` and `text_verify` fingerprint the installed LaMa, font
  and PP-OCRv5 files; the replacement steps, the read-back check, `text_typeset`, `output` and
  `localized_video` read the `localized_video` setting, which the translation step's fingerprint
  leaves out. A text correction reruns the replacement steps, the check, the output and the
  localized video, never the audio steps.

## Fallbacks

Each occurrence that is not baked stays Japanese in the localized video, with one of these
reasons in `outputs/text_verify` and Check Text:

| Reason | Step | Cause |
|---|---|---|
| Nearby placement was chosen in Check Text | mask | the owner reviewed it with the Nearby treatment |
| No video frame starts while the writing is shown | mask | the occurrence falls between two frames |
| The writing's position in the frame is unknown | mask | no keyframe quad, or no plate inside the frame |
| The writing could not be separated from its background | mask | no partition reads as lettering, as printing on a panel or as outlined lettering along the box's edges; writing the box cuts off; implausible coverage; or a loose box's ink covering less than half of it |
| Japanese strokes reach outside the erase area | mask | ink-coloured strokes apart from the mask inside the quad over 8 % of the mask's area |
| The writing moves in a way that could not be followed | mask | a frame's correlation under 0.8, or a sweep over a quarter of the frame |
| The background changes too often to repaint | mask | more than 2,000 background runs |
| The writing is no longer in the reviewed text document | compose | the occurrence left the reviewed document after the mask step |
| The original lettering style was not measured | compose | the document carries no style for it |
| The writing has no English translation | compose | its English is missing or blank |
| The font has no glyph for “…” | compose | a character Noto Sans cannot draw |
| The writing has no background plates | compose | the document carries no plate for it |
| The writing has no tracked position at its keyframe | compose | no observed frame with a valid quad |
| Another replacement covers this writing | compose | another occurrence on screen together is kept for the same sign |
| The English would be too small to read in place | compose | cap height under 14 pixels at 1080p |
| The writing's position cannot be mapped onto its background | compose | the keyframe quad gives no usable perspective on a plate |
| The finished picture still shows Japanese | verify | in a sampled finished frame, a reading of 2 or more kana or kanji at 0.5 confidence where the writing or its furigana was |
| The English does not read back cleanly | verify | in a sampled finished frame, the letters read over the lettering are less than 0.6 similar to the English |

The `visual_validation` tool's `mask-probe` prints every figure the mask step judges an
occurrence of a finished job by, reruns the whole step with `--all` against the job's verdicts,
and `residue-probe` prints each filled plate's residue share; both run on the CPU.
`verify-probe` runs the read-back check over a finished job on the GPU and prints what it read.

Decode, model and file errors are not fallbacks: they fail the step, which Try Again resumes.

## Boundaries

- Depends on: `job_model` contracts, `media_io` region crops, native frames and the encoder,
  `inference::onnx::lama`, `image`, `imageproc`, `tiny-skia` and `ttf-parser`; FFmpeg and ffprobe
  as child processes.
- Used by: `pipeline::tasks::replace` and `pipeline::tasks::localized`; the window's Check Text
  and Overview read the documents and files.
- Rules: the source video is only read; only masked pixels change; frames stream and are never
  held for a whole occurrence; an occurrence that cannot be replaced falls back with a reason
  rather than a guess; the localized video never carries a subtitle stream.

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#replacement-and-the-localized-video) — the
  step table and where these steps sit.
- [System overview](/documentation/architecture/system_overview.md) — workers, the GPU lock, the
  job database and the work directory.
- [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md) — detection,
  translation, review and what the owner sees.
- [Localized video on Dressrosa 11](/documentation/research/localized_video_dressrosa_11.md) —
  time, memory, size and results on one episode.
- [In-place replacement decision](/documentation/decisions/stack_and_pipeline.md#2026-09-30--writing-is-replaced-in-a-localized-video-re-encoded-beside-the-source).
