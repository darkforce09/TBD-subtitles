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
├── mod.rs     the `RegionSource` contract: frame timeline, frame size and region decoding
├── source.rs  `FfmpegRegions`: full-resolution region crops decoded by FFmpeg
└── tests/     region decoding checks for `FfmpegRegions`
```

## How it works

The pipeline runs three steps in order, each reading the previous step's `ReplacementDocument`
and writing its own. `mask::extract` decodes only the regions around each occurrence through a
`RegionSource`, measures strokes and style, and writes masks and source plates under
`visual/masks/`. `inpaint::inpaint` fills each plate's masked pixels through the `Inpaint` backend,
which the pipeline runs as LaMa in an ONNX Runtime worker. `compose::compose` letters the English
onto each filled plate. Frame numbers index the decoded frame timeline from zero; every path in a
document is relative to the job directory.

## Boundaries

- Depends on: `job_model` replacement and visible-text contracts, `media_io` and FFmpeg for
  region decoding, `image` and `imageproc`, and the sibling `geometry` and `glyphs` modules.
- Used by: `pipeline::tasks::replace` for the mask, inpaint and compose steps.
- Rules: source videos are only read; only masked pixels are ever replaced; frames are streamed
  and never held for a whole occurrence; a stage that cannot handle an occurrence falls back with
  a reason instead of guessing.

## Related documentation

- [Video inpainting](/documentation/architecture/video_inpainting_pipeline.md) — the in-place replacement architecture.
- [Pipeline](/documentation/architecture/pipeline.md) — step order, workers and resume.
