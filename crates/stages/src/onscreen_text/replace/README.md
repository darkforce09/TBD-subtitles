# In-place text replacement

The stages that draw translated writing into the picture itself. Each translated occurrence is
erased at the stroke level, its background is filled by inpainting, and English lettering in the
measured style is composed onto the filled plate for the localized video. An occurrence any stage
cannot handle keeps its English in the subtitle file, with the reason recorded.

## Contents

```text
crates/stages/src/onscreen_text/replace/
├── compose/   English lettering composed onto inpainted plates, and review previews
├── inpaint/   the inpainting contract and the pass that fills every plate's erased strokes
├── mask/      stroke masks, lettering style, per-frame placement and background plates
├── mod.rs     the `RegionSource` contract and which occurrences Claude found (`found_by_claude`)
├── source.rs  `FfmpegRegions`: full-resolution YUV region crops, converted in the stream's colour
├── verify/    the read-back check: a local OCR approves each lettered replacement from its frames
└── tests/     region decoding checks for `FfmpegRegions` and Claude-found ids
```

## How it works

The pipeline runs four steps in order. The first reads the reviewed `TextDocument`; each later
one reads the previous step's `ReplacementDocument`, and each returns its own for the pipeline to
store. `mask::extract` decodes only the regions around each occurrence through a
`RegionSource`, measures strokes and style, writes masks and source plates under
`visual/masks/`, and hands every frame's placement to the caller as a `FrameRecord`, which the
pipeline stores as the job's `frames` rows. `inpaint::inpaint` fills each plate's masked pixels
through the `Inpaint` backend, which the pipeline runs as LaMa in an ONNX Runtime worker.
`compose::compose` letters the English onto each filled plate at every shift the `frames` rows
give (a `localize::motion::Motion` the pipeline folds from them). `verify::verify` rebuilds
sampled frames of each baked occurrence as the localized video shows them and has PP-OCRv5 read
them back, sending back to Japanese any whose finished picture still shows Japanese or whose
English does not read back; the pipeline stores its readings as the `readings` rows. Frame numbers index
the decoded frame timeline from zero; every path in a document is relative to the job directory.
An occurrence whose id ends in `-c` and a number was found by Claude on a keyframe: its one quad
is Claude's loose box, which the mask step pads and refits to the ink and the compose step ranks
below the detector's own occurrences of one sign.

`source::FfmpegRegions` is the production `RegionSource`, which the mask and verify steps
read. It reads the timeline once and takes the stream's matrix and range from the probe
(`Coefficients::of`). Each `frames` call starts one `media_io` `RegionStream`: FFmpeg seeks to the
span's first frame and crops `yuv420p` to the even-aligned rectangle around the region, half the
bytes of rgb24. Rust then trims each crop to the region and converts it to rgb24 in the stream's
own colour. The decode and conversion run on their own thread through a bounded `FrameQueue`, with
the YUV buffers recycled from a pool, so FFmpeg keeps decoding while the visitor works. A visitor
that returns an error ends the decoder. The crops agree with FFmpeg's own rgb24 conversion to
within a few levels, so the mask, inpaint and verify steps' revisions count the change.

## Boundaries

- Depends on: `job_model` replacement and visible-text contracts, `media_io` and FFmpeg for
  region decoding, `inference::ocr` for the read-back check, `image` and `imageproc`, `tiny-skia`
  and `ttf-parser` for the lettering, the sibling `geometry`, `png` and `detect` modules, and
  `localize` for the per-frame shifts and the finished picture.
- Used by: `pipeline::tasks::replace` for the mask, inpaint and compose steps and
  `pipeline::tasks::verify` for the read-back check; the `visual_validation` tool's `mask-probe`,
  `residue-probe` and `verify-probe`.
- Rules: source videos are only read; only masked pixels are ever replaced; frames are streamed
  and never held for a whole occurrence; a stage that cannot handle an occurrence falls back with
  a reason instead of guessing.

## Related documentation

- [Video inpainting](/documentation/architecture/video_inpainting_pipeline.md) — the in-place replacement architecture.
- [Pipeline](/documentation/architecture/pipeline.md) — step order, workers and resume.
