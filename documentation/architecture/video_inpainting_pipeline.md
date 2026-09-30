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
text_review ─▶ text_mask ─▶ text_inpaint ─▶ text_compose ─▶ text_typeset ─▶ qc ─▶ output ─▶ localized_video
  reviewed      stroke masks  LaMa fills     English          ASS events      <video>.ass        <video>.localized.mkv
  English       source plates the plates     patches          + events not    <video>.localized.ass
                                                              drawn in
```

| Step | Runs in | Reads | Writes |
|---|---|---|---|
| `text_mask` | worker, `tbd-subtitles` (CPU; FFmpeg region crops) | `probe.json`, `visual/text_review.json`, the source video | `visual/text_mask.json`, `visual/masks/` |
| `text_inpaint` | worker, `tbd-subtitles` (ONNX Runtime, GPU lock) | `visual/text_mask.json` and its PNGs | `visual/text_inpaint.json`, `visual/plates/` |
| `text_compose` | worker, `tbd-subtitles` (CPU) | `visual/text_review.json`, `visual/text_inpaint.json`, the `latin-fonts` model | `visual/text_compose.json`, `visual/patches/` |
| `text_typeset` | worker, `tbd-subtitles` (CPU) | `visual/text_review.json`, `visual/text_compose.json` | `visual/text_typeset.json`, `visual/events.ass`, `visual/events_localized.ass` |
| `output` | job runner | the cues, both event files | `<video>.ass` (or the chosen format), `<video>.localized.ass`, `output.json` |
| `localized_video` | worker, `tbd-subtitles` (FFmpeg decoder and encoder, GPU lock) | `probe.json`, `visual/text_compose.json` and its patches, the source video | `<video>.localized.mkv`, `visual/localized_video.json` |

The three replacement steps pass one `ReplacementDocument`
([contract](/crates/job_model/src/onscreen/localize.rs)) from step to step, each adding to it:
one `ReplacedText` per candidate occurrence with its frame span, its status (`pending`, `baked`
or `fallback` with a reason), its measured `LetteringStyle`, its container and its
[plates](/documentation/glossary.md#plate). Frame numbers index the video's presentation
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
  frame to RGB and crops it. The analysis window is the keyframe quad's bounds grown by 4 pixels;
  the plate is its bounds grown by `max(32, 0.75 × the shorter side)`, the context inpainting
  sees.
- **Separating strokes:** the window's pixels are partitioned in CIE Lab with k = 2 to 5
  (k-means, at most 20 iterations, centres fitted on at most 65,536 pixels). Each partition is
  read first as lettering over the background the window's 3-pixel border ring shows: colours
  that dominate the ring, or are no rarer there than inside, are background; a core ink colour
  sits at least 2.0 spreads from every background colour; a fill wrapped by an outline counts even
  on a noisy translucent panel, and only the outline band around the fill is ink, so line art of
  the outline's colour stays. When no partition reads as lettering, it is read as writing printed
  on a panel or sign that fills the box. Strokes the detector's box clips are followed across the
  plate by at most `max(8, half a line)` pixels, furigana above outlined lines are added, and
  writing the box cuts off falls back. A partition passes when its ink covers 1 % to 55 % of the
  quad; among the nearly cleanest passing partitions the one covering most of the quad wins. The
  [stroke mask](/documentation/glossary.md#stroke-mask) is the ink dilated by
  `max(2, 0.06 × line height)` pixels, 255 where pixels are erased; panels and pictures are never
  marked. The full rules: [mask stage](/crates/stages/src/onscreen_text/replace/mask/).
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
- **Files:** `visual/plates/<occurrence>/<n>.png`, written under a temporary name and renamed. A
  plate whose source and mask files match an earlier plate's byte for byte reuses its result,
  from a cache of at most 64 plates and 256 MiB. One plate is decoded at a time; a plate with an
  empty mask is its source, with no model call.

## Lettering (`text_compose`)

Code: [compose](/crates/stages/src/onscreen_text/replace/compose/).

- **Font:** Noto Sans, the variable font with width and weight axes (`NotoSans.ttf`, OFL, model
  folder `latin-fonts`), read with `ttf-parser` and rasterized with `tiny-skia`, both pure Rust.
  A character the font cannot draw falls back.
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

## The localized subtitle file

`text_typeset` writes two event files. `visual/events.ass` holds every displayable occurrence as
before and goes into `<video>.ass`. `visual/events_localized.ass` holds only the occurrences not
baked into the video, each typeset by the same rules (ASS lettering on a safe surface, else a
nearby label), and each fallback gains the warning `Not replaced in the video: <reason>` in
`visual/text_typeset.json`, the report and Check Text. The output step writes the dialogue and
sound cues plus those events as `<video>.localized.ass`, backing up a different existing file
into the job's `backup/` folder, and records it as `localized` in `output.json`.

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
- **Record:** `visual/localized_video.json` holds the path, the encoder, the frames written and
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
- Resume: `text_inpaint` and `text_compose` fingerprint the installed LaMa and font files; the
  replacement steps, `text_typeset`, `output` and `localized_video` read the `localized_video`
  setting, which the translation step's fingerprint leaves out. A text correction reruns the
  replacement steps, the output and the localized video, never the audio steps.

## Fallbacks

Each occurrence that is not baked keeps its English in `<video>.localized.ass` with one of these
reasons:

| Reason | Step | Cause |
|---|---|---|
| Nearby placement was chosen in Check Text | mask | the owner reviewed it with the Nearby treatment |
| No video frame starts while the writing is shown | mask | the occurrence falls between two frames |
| The writing's position in the frame is unknown | mask | no keyframe quad, or no plate inside the frame |
| The writing could not be separated from its background | mask | no partition reads as lettering or as printing on a panel, writing the box cuts off, or implausible coverage |
| The writing moves in a way that could not be followed | mask | a frame's correlation under 0.8, or a sweep over a quarter of the frame |
| The background changes too often to repaint | mask | more than 2,000 background runs |
| The English would be too small to read in place | compose | cap height under 14 pixels at 1080p |
| The font has no glyph for “…” | compose | a character Noto Sans cannot draw |
| The writing's position cannot be mapped onto its background | compose | the keyframe quad gives no usable perspective on a plate |

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
- [System overview](/documentation/architecture/system_overview.md) — workers, the GPU lock and
  the work directory.
- [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md) — detection,
  translation, review and what the owner sees.
- [Localized video on Dressrosa 11](/documentation/research/localized_video_dressrosa_11.md) —
  time, memory, size and results on one episode.
- [In-place replacement decision](/documentation/decisions/stack_and_pipeline.md#2026-09-30--writing-is-replaced-in-a-localized-video-re-encoded-beside-the-source).
