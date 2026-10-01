# Inpainting

The replacement step between stroke masks and lettering. For every occurrence still pending, it
fills each plate's erased strokes from their surroundings with a square inpainting model (LaMa in
the pipeline) and records the filled plate for composition.

## Contents

```text
crates/stages/src/onscreen_text/replace/inpaint/
├── fill.rs     one plate through the model: working size, mirrored padding, tiles, feathered blend
├── mod.rs      the `Inpaint` trait, the step over the document, plate files and the bounded cache
├── residue.rs  lettering the fill still shows under the mask, and the wider retry mask
└── tests/      size regimes, padding and tiles, blending, skips, cache reuse, residue and failures
```

## How it works

Each plate's source crop (RGB PNG) and erase mask (8-bit PNG) are read from the job directory and
must match the plate's rectangle. A mask without a set pixel needs no model: the plate is the
source. Otherwise the plate is brought to the model's side:

- a plate that fits sits at the top-left of the square;
- a plate whose longest side is at most twice the model side is scaled so that side matches;
- a larger plate is halved and covered by square tiles whose neighbours overlap by at least a
  quarter of the side (128 pixels for LaMa); tiles blend linearly across each overlap, and a tile
  without a masked pixel costs no model call.

The square past the picture's edge mirrors the picture, and the mask mirrors with it, so mirrored
strokes are erased rather than offered to the model as context. A scaled mask sets a pixel when
any pixel it covers is set and then grows by one pixel, so no stroke is lost and no ink bleeds
back through the scaling. The filled picture is scaled back with a triangle filter and blended
onto the source: fully on masked pixels, and by the share of masked pixels in each 3 × 3 box
around the others, so the fill fades out over one pixel and everything further away is the source
byte for byte.

Every filled plate of an occurrence with a measured lettering style is checked for residue: a
masked pixel still looks like the lettering when it lies within ΔE 12 of the measured fill colour
and more than ΔE 20 from the median of the filled plate around it (a 9 × 9 grid spanning a
quarter of a line on each side, at least 4 pixels), in a blob that survives an opening by an
eighth of the stroke thickness, so thin joints and cracks the fill continues do not count. When
more than 3 % of the mask's pixels do, the plate is filled once more with the mask grown by a
tenth of a line; that mask is written as `<plate index>-mask.png` beside the plate and becomes
the plate's mask, so the patch covers it too. The wider fill stands even when it is still above
3 %: whether the finished replacement is clean is decided by the
[read-back check](/crates/stages/src/onscreen_text/replace/verify/), which reads the finished
picture, not by a pixel rule on one plate. `residue_share` is public for the `visual_validation`
tool's `residue-probe`.

Plates go to `visual/plates/<occurrence id>/<plate index>.png`, written under a temporary name and
moved into place; the id keeps ASCII letters, digits, `-` and `_`, and colliding names get a
numbered suffix. Plates whose source and mask files are byte-identical to an earlier plate's in
the same run reuse its result: the cache holds at most 64 plates and 256 MiB, oldest dropped
first. One plate is decoded at a time.

## Boundaries

- Depends on: `image` for PNG and resizing, `imageproc` for the residue's opening,
  `job_model::onscreen` replacement contracts, the sibling `mask` module's colour distance
  (`delta_e`, `INK_DELTA_E`) and `onscreen_text`'s `png` module for synced plate files.
- Used by: `pipeline::tasks::replace`, which supplies LaMa through `inference::onnx::lama`;
  `residue_share` by the `visual_validation` tool's `residue-probe`.
- Rules: occurrences not `Pending` stay untouched; pixels further than one pixel from a mask keep
  their source bytes (`every_size_regime_fills_the_mask_and_keeps_the_rest`); a missing or
  mis-sized file and a model error fail the step.

## Related documentation

- [Video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md) — the
  replacement steps and their outputs.
- [LaMa inpainting](/crates/inference/src/onnx/lama/) — the model behind the `Inpaint` trait.
