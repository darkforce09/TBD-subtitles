# Stroke masks

The first replacement step. For every displayable translated occurrence it separates the
writing's strokes from their background on the keyframe, measures the lettering style, follows
moving writing frame by frame, and cuts the occurrence's span into background plates that later
steps inpaint and letter. Writing that cannot be separated, followed or repainted falls back to
the subtitle file with a reason.

## Contents

```text
crates/stages/src/onscreen_text/replace/mask/
├── cluster.rs      sRGB to CIE Lab, k-means partitions with two to five clusters, separation
├── complete.rs     completion: joined stroke ends, strays, a loose box's refit, the dilated mask
├── correlation.rs  zero-mean normalized cross-correlation over grey planes and running sums
├── files.rs        occurrence folder names, atomic PNG writes and clearing stale masks
├── follow.rs       per-frame placement of moving writing: prediction, search, still or moving
├── ink.rs          which clusters are writing: lettering over the ring, ring outlines
├── mod.rs          `extract`: candidates, fallbacks, keyframe decode and the document
├── panel.rs        which clusters are writing printed on a panel filling the box
├── pieces.rs       connected ink pieces: kept, cut, furigana, frames, beside a loose box
├── plates.rs       a span streamed into runs, the correlation of still writing, the frame rows
├── probe.rs        `diagnose`: every figure segmentation and following judge an occurrence by
├── reach.rs        ink outside the window: strokes the box clips and furigana above a line
├── runs.rs         background runs: per-frame masks, the overlap split, union plates, the cap
├── segment.rs      readings, partition choice, coverage and cut-off guards
├── select.rs       candidates, frame spans, keyframe quad, static check and rectangles
├── style.rs        fill and outline roles and colours, stroke and outline thickness, softness
└── tests/          synthetic frames through a scripted region source, and unit checks
```

## How it works

A candidate has non-empty English, is reviewed or has a confidence of at least 0.85, and carries
observed frames and a keyframe. Its span is every frame whose start lies in `[start_s, end_s)`
(1 ms tolerance). A Nearby choice in Check Text, or a span without frames, falls back at once.

The keyframe quad is the observation at the keyframe time. The window is its bounds grown by 4
pixels, the template moving writing is followed by. The analysis window is the bounds of the quad
and of the furigana boxes folded into the line (`ruby`) grown by 4 pixels; the plate is those
bounds grown by `max(32, 0.75 × shorter side)`, the context inpainting sees. A loose box Claude
found on a keyframe (an id ending in `-c` and a number) grows both by a further 0.35 of its
height, and when the padded analysis does not separate, the window alone is tried. Only the
keyframe's plate is decoded for segmentation. The analysis window's pixels are partitioned in Lab
four times, with k = 2, 3, 4 and 5 (starts at lightness quantiles and at farthest points, at most
20 iterations, centres fitted on at most 65,536 evenly strided pixels).

Each partition is first read as lettering over the background the window's 3-pixel border ring
shows. A cluster is background when it holds more than 30 % of the ring, or when its share of the
ring exceeds 0.75 of its share of the window's inside (texture is everywhere, writing sits inside
the box). A core colour of the writing is a non-background cluster of at least 0.5 % of the window
that sits at least 2.0 spreads from every background cluster. Outlined lettering on a
translucent panel may sit closer: two non-background clusters at least 1.0 spread from the
background and 2.0 from each other are fill and outline when at least 80 % of one lies within the
dilation radius of the other and at least half of the other lies within it of the first (the one
touching the background less is the fill); only the outline's pixels within that radius of the
fill count, so picture line art of the outline's colour stays. A weak cluster with at least 85 %
of its pixels within the radius of core ink is the writing's fringe and is ink too; any other is
background. A cluster lying strictly between a core colour and a background colour (beyond a
quarter of the way from each, within a quarter of their distance of the line) is anti-aliasing,
ink but never fill or outline. A colour the ring does not show that covers half the quad, or
30 % of it without being ink, is a panel and ends the lettering reading.

When no partition passes as lettering, or the chosen one holds no fill and outline wrapping each
other, the partitions whose lettering reading holds such a pair and separates by at least 2.0
spreads are read again as dense lettering, which must cover more than 55 % and at most 80 % of the
quad: bold writing with a heavy outline fills a box drawn tight around it. A passing dense
partition replaces the plain choice. With no choice yet, each partition is read as writing printed
on a panel or sign that fills the box: the cluster covering at least half the quad with 80 % of
it in one 4-connected piece is the panel, and every colour 2.0 spreads from it is ink. When
neither passes and the box is the detector's, each partition is read as outlined lettering whose
colours also run along the box's edges: a fill and outline wrapping each other, whatever their
ring share, both with a spread of at most 15 (or a fifth of the distance between them, when they
stand at least 1.25 spreads rather than 1.0 from every other ring colour), and with at least half
the fill's pixels having three of their four neighbours in the fill, are the only core colours.
Lettering crossing the ring crowds the background out of it, so the other colours are judged
background again over the ring and window with the pair's pixels set aside, by the same shares as
above; a picture colour behind a translucent card then no longer reads as a hidden panel.

Ink is cut into 8-connected single-colour pieces; pieces smaller than
`max(4, 0.05 % of the window)` pixels are dropped, and pieces touching the window border are
kept only when followed: the same colour, classified by the window's centres within three
spreads of a centre, continues across the plate no further than `max(8, 0.5 × line height)`
pixels past the window, never to the plate's border, and lies no more outside the window than
inside it. Ink pixels inside a furigana box are kept even where the border cuts their piece. Of
the kept ink, a fill piece of an outlined pair with less than 80 % of its edge within the
dilation radius of the outline is picture in the fill's colour and dropped; a piece spanning more
than 2.5 line heights one way and 0.8 the other, filling less than 35 % of its bounds and
enclosing half the other ink is a frame and dropped; for a loose box, a piece with less than a
third of its pixels inside the box is dropped; followed strokes stay only while joined to the
ink that is left. A partition passes when
its ink covers 1 % to 55 % of the line's quad (80 % for dense lettering), when unfollowed pieces the border cuts (reaching
`max(6, 0.1 × line height)` pixels into the window; for a panel, within its extent) hold at most
20 % of the ink inside the quad and furigana boxes, and when one piece spans at least 0.3 of a
line height. Of the passing partitions nearly as clean as the cleanest (at least 0.9 of its
weakest core separation), the one covering most of the quad and furigana wins; with none the
occurrence falls back.

Without furigana boxes, outlined lettering has its furigana added (on a panel too): in the band
of 0.6 line heights above the window, pieces of outline plus fill blobs lying at least 90 % within
2 pixels of the outline (and the outline hugging them) that stay clear of the band's top and
sides and are at most half a line tall.

Completion (`complete.rs`) then takes a pixel as ink-coloured when its nearest cluster is ink and
it lies within ΔE 12 of the measured fill or outline. Within half a line around the analysis
window, 8-connected pieces of ink-coloured pixels outside the ink that touch it, stay clear of
that region's edge and hold at most a quarter of the ink's area and a stroke's width (outline
included) times a line height join it; pieces reaching the edge are picture. Pieces apart from
the ink and clear of the edge are strays; strays inside the quad and furigana boxes over 8 % of
the dilated mask's area fall back as "Japanese strokes reach outside the erase area". For a
loose box, the bounds of the ink inside the analysis window become the lettering area
(`lettering_quad`), and ink covering less than half of the box falls back as unseparated. The mask
is the completed ink dilated by `max(2, 0.06 × line height)` pixels, 255 where strokes are erased,
over the whole plate; the background and any panel are never marked.

`diagnose` runs the same keyframe location and segmentation for one occurrence and returns every
partition's reading, coverage, cut share, largest piece and failed guard, the completion counts,
the refitted area and the mask, and for writing that separates and moves, every frame's best match
and whether the writing counts as still, moving or unfollowed; the `visual_validation` tool's
`mask-probe` prints them.

Fill and outline are the core cluster with the most ink and the next core cluster at least 2.0
spreads from it; the one nearer the outside of the ink is the outline. The fill colour is the
median of the fill eroded by one pixel; stroke thickness is twice the fill area over its
perimeter, and outline thickness the outline area over the same perimeter. An outline is soft when
the band just outside it departs from the background colour clearly more often than the window's
border ring does.

Writing whose observed quads all sit within half a pixel of the keyframe quad is static, and its
plate rectangle is decoded once for the whole span. Moving writing is followed first: the
keyframe window, in grey at scales 0.9 to 1.1, is matched by zero-mean normalized
cross-correlation around the position interpolated from the sampled quad centres, within
`0.5 × shorter side + 16` pixels, coarse on block averages and then at full size around the best
two coarse matches. When every frame scores at least 0.8, the plates follow the placements. A
sign whose sampled quads only jitter (the detector box grows and shrinks around writing that stays
put) is still: when every frame scoring 0.8 sits at the keyframe placement, those frames are at
least 90 % of the span, and the rest still match somewhere by 0.6 (a streak or flash crossing the
sign), its plates are collected as static writing's. Any other frame under 0.8 loses the
occurrence. The region moving plates sweep is decoded once; more than a quarter of the frame
falls back.

Every frame's erase mask is the keyframe mask carried to its placement. A frame joins the current
run while it keeps the run's scale, its mask overlaps the union of the run's masks by an
intersection over union of at least 0.85 (`MIN_MASK_IOU`), and the plate pixels both masks keep
differ from the run's first frame by a mean under 3 and a 99th percentile under 24 per channel
value. Each run is one `Plate`: the union of its frames' rectangles, its source the first frame,
its erase mask the union of its frames' masks, its shift and scale the first frame's. Writing that
keeps one placement therefore gets the plates a split per placement gives. Once an occurrence has
its plates, every frame of its span goes to the caller's `FrameSink` as one `FrameRecord`: the
keyframe quad carried to the frame, the correlation there (the tracker's score for moving writing;
for still writing the keyframe window against the frame's, 0 without contrast), the shift and
scale, the run-length mask relative to its plate and the plate index. A second occurrence with an
id already given rows falls back, so rows never collide. Files live in `visual/masks/<occurrence>/`:
`mask.png` for the keyframe placement, `mask-<n>.png` for the union mask of every other plate,
and `source-<n>.png` per plate. The mask folder is emptied at the start of every
extraction, and an occurrence that falls back after writing files loses its folder. More than
2,000 plates for one occurrence falls back.

## Boundaries

- Depends on: the parent's `RegionSource`, `job_model` replacement contracts, `image` and
  `imageproc`, and the `geometry` and `png` modules of `onscreen_text`.
- Used by: `pipeline::tasks::replace` for the stroke-mask step, which stores each `FrameRecord`
  the `FrameSink` receives as a `frames` row; `diagnose` by the
  `visual_validation` tool's `mask-probe`.
- Rules: source pixels are only read; only the current run's first frame, its union mask and the
  frame being examined are held, with a log of a few bytes per frame from which the rows are sent
  one at a time; decode and file errors fail the step while visual problems fall back; the
  document validates before it is returned.

## Related documentation

- [Video inpainting](/documentation/architecture/video_inpainting_pipeline.md) — the in-place replacement architecture.
- [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md) — detection, review and presentation of visible writing.
