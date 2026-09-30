# Lettering composition

The last pixel step of in-place replacement. It letters each pending occurrence's English onto
its inpainted plates in the original writing's place, colour and weight, and writes one RGBA
patch per plate that the localized video blends over the source frames, or records why the
English stays in the subtitle file instead.

## Contents

```text
crates/stages/src/onscreen_text/replace/compose/
├── bake.rs        per-occurrence files: render once, warp onto each plate, write patches and preview
├── colours.rs     measured fill and outline, WCAG contrast and the added legibility outline
├── containers.rs  keyframe quads, plate placement and grouping of neighbouring writing
├── font.rs        the Noto Sans variable font: metrics per width and weight, glyph paths
├── layout.rs      line breaking, size and width-axis fitting, line placement, shared scale
├── mod.rs         the compose entry point: preparation, containers, fitting and fallbacks
├── patch.rs       feathered masks, lettering over the plate, the preview and atomic PNG writes
├── render.rs      supersampled tiny-skia rasterizing with outline under fill and soft outlines
├── warp.rs        inverse-homography bilinear warp of the canvas onto a plate
└── tests/         layout, grouping, colour, rendering, warp, patch and end-to-end checks
```

## How it works

`compose` opens `NotoSans.ttf` from the `latin-fonts` model folder; a missing or unreadable font
is an error. Each `Pending` occurrence takes its English from the reviewed text document and its
keyframe quad from the tracked frame whose interval holds the keyframe time (else the nearest).
Missing English, an unmeasured style, no plates, no tracked position or a character the font
cannot draw make it fall back with a reason.

Occurrences on screen together whose keyframe quads, each grown by one original line height,
touch form one container (union-find in document order); every member records the first
member's id. The target cap height is 0.7 times the original line height. Layout works in the
rectified quad (width the mean of the top and bottom edges, height the mean of the sides): the
English wraps greedily at spaces, or one word per line when the area is more than 1.6 times
taller than wide, and must fit 92 % of the width and 90 % of the height with 1.15 em line
spacing. At each size the width axis tries 100, then 87.5, then 75 before the size shrinks by 4 %.
Members of a container share the largest factor `k ≤ 1` at which each fits at `k` times its own
target, so their sizes keep the source ratio. A cap height below 14 pixels per 1080 frame lines
falls back as too small to read. The weight axis follows stroke thickness over line height: under
0.08 is 400, under 0.12 is 600, under 0.16 is 700, else 900.

The fill is the measured fill; a measured outline keeps its colour at no less than 1 pixel. With
no outline, a fill whose WCAG contrast with the mean plate colour under the writing is below 3:1
gets a 2-pixel black or white outline, whichever contrasts more with the fill. Glyph outlines are
rasterized with tiny-skia on a canvas at twice the source resolution (at most 4096 pixels a side):
the outline is a round-joined stroke twice its width, drawn under the fill, box-blurred when soft.

Each plate places the keyframe quad scaled about its centre by the plate's scale, moved by its
shift and made local to its rectangle. Every plate pixel maps back through the inverse homography
into the canvas and samples it bilinearly. The patch's colour is the lettering over the inpainted
plate everywhere; its alpha is the larger of the 3 by 3 feathered erase mask and the lettering's
coverage. Patches go to `visual/patches/<id>/<n>.png`, and `preview.png` shows the keyframe
plate's patch over its original pixels; every file is written to a temporary name and renamed.
Once every plate has its patch, the occurrence is `Baked`.

## Boundaries

- Depends on: `job_model` replacement and text contracts, the sibling `geometry` homographies,
  `image` for PNG files, `tiny-skia` for rasterizing and `ttf-parser` for the variable font.
- Used by: `pipeline::tasks::replace` for the text-compose step.
- Rules: only `Pending` occurrences change; a fallback leaves no patch paths and no patch folder;
  source images are only read; one plate's images are held at a time; output is deterministic.

## Related documentation

- [Video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md) — masks,
  inpainting, lettering and the localized video.
- [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md) — detection,
  review and presentation of visible writing.
