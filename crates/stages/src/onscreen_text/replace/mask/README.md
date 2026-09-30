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
├── correlation.rs  zero-mean normalized cross-correlation over grey planes and running sums
├── files.rs        occurrence folder names, atomic PNG writes and clearing stale masks
├── follow.rs       per-frame placement of moving writing: prediction, coarse and fine search
├── ink.rs          which clusters are writing: lettering over the ring's background, or a panel
├── mod.rs          `extract`: candidates, fallbacks, keyframe decode and the document
├── plates.rs       background runs, moved masks, source plates and the plate cap
├── reach.rs        ink outside the window: strokes the box clips and furigana above a line
├── segment.rs      partition choice, ink pieces, coverage and cut-off guards, the dilated mask
├── select.rs       candidates, frame spans, keyframe quad, static check and rectangles
├── style.rs        fill and outline roles and colours, stroke and outline thickness, softness
└── tests/          synthetic frames through a scripted region source, and unit checks
```

## How it works

A candidate has non-empty English, is reviewed or has a confidence of at least 0.85, and carries
observed frames and a keyframe. Its span is every frame whose start lies in `[start_s, end_s)`
(1 ms tolerance). A Nearby choice in Check Text, or a span without frames, falls back at once.

The keyframe quad is the observation at the keyframe time. The analysis window is its bounds
grown by 4 pixels; the plate is its bounds grown by `max(32, 0.75 × shorter side)`, the context
inpainting sees. Only the keyframe's plate is decoded for segmentation. The window's pixels are
partitioned in Lab four times, with k = 2, 3, 4 and 5 (starts at lightness quantiles and at
farthest points, at most 20 iterations, centres fitted on at most 65,536 evenly strided pixels).

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

When no partition passes as lettering, each is read as writing printed on a panel or sign that
fills the box: the cluster covering at least half the quad with 80 % of it in one 4-connected
piece is the panel, and every colour 2.0 spreads from it is ink.

Ink is cut into 8-connected single-colour pieces; pieces smaller than
`max(4, 0.05 % of the window)` pixels are dropped, and pieces touching the window border are
kept only when followed: the same colour, classified by the window's centres within three
spreads of a centre, continues across the plate no further than `max(8, 0.5 × line height)`
pixels past the window, never to the plate's border, and lies no more outside the window than
inside it. A partition passes when its ink covers 1 % to 55 % of the quad, when unfollowed pieces
the border cuts (reaching `max(6, 0.1 × line height)` pixels into the window; for a panel, within
its extent) hold at most 20 % of the ink inside the quad, and when one piece spans at least 0.3 of
a line height. Of the passing partitions nearly as clean as the cleanest (at least 0.9 of its
weakest core separation), the one covering most of the quad wins; with none the occurrence falls
back.

For outlined lettering over a background, furigana are added: in the band of 0.6 line heights
above the window, pieces of outline plus fill blobs lying at least 90 % within 2 pixels of the
outline (and the outline hugging them) that stay clear of the band's top and sides and are at most
half a line tall. The mask is the ink, followed strokes and furigana dilated by
`max(2, 0.06 × line height)` pixels, 255 where strokes are erased, over the whole plate; the
background and any panel are never marked.

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
two coarse matches. A frame whose best score is under 0.8 loses the occurrence. The plates then
follow the placements, and the region they sweep is decoded once; more than a quarter of the frame
falls back.

A frame joins the current run while its placement is unchanged and the plate pixels the mask keeps
differ from the run's first frame by a mean under 3 and a 99th percentile under 24 per channel
value. Each run is one `Plate` whose source is its first frame. Files live in
`visual/masks/<occurrence>/`: `mask.png` for the keyframe placement, `mask-<n>.png` for each other
placement, and `source-<n>.png` per plate. The mask folder is emptied at the start of every
extraction, and an occurrence that falls back after writing files loses its folder. More than
2,000 plates for one occurrence falls back.

## Boundaries

- Depends on: the parent's `RegionSource`, `job_model` replacement contracts, `image` and
  `imageproc`, and the sibling `geometry` module.
- Used by: `pipeline::tasks::replace` for the stroke-mask step.
- Rules: source pixels are only read; only the current run's first frame and the frame being
  examined are held; decode and file errors fail the step while visual problems fall back; the
  document validates before it is returned.

## Related documentation

- [Video inpainting](/documentation/architecture/video_inpainting_pipeline.md) — the in-place replacement architecture.
- [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md) — detection, review and presentation of visible writing.
