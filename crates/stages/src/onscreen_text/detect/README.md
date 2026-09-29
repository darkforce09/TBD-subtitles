# Text detection

The first visual stage. It screens a small copy of the video for visible writing, finds each
region's exact first and last frame, and confirms every occurrence once on a full-resolution
still, which yields its crop and its keyframe image.

## Contents

```text
crates/stages/src/onscreen_text/detect/
├── crops.rs    rectified crops, plain-surface checks and keyframe confirmation on full-size stills
├── mod.rs      the scan: batched screening, region following, exact transitions and the document
├── regions.rs  fixed-anchor signatures, mutually unique matches and tiled occurrence frames
├── screen.rs   the sample schedule, repeated samples, pending batches and lockstep bisection
├── source.rs   the frame source contract, proxy frames and the FFmpeg source
└── tests/      association, rectification, sampling, bisection and whole-scan checks
```

## How it works

`FfmpegSource` decodes a proxy copy `PROXY_LINES` (360) rows high with the loop filter skipped,
and reads full-resolution stills by frame index through accurate seeks, four at a time. The scan
treats every `k`-th frame as a sample, where `k` is half the frame rate rounded, together with both
frames around every shot cut and the final frame, so the frames between two samples never cross a
cut. Samples wait in batches of four with the frames since the previous sample. One detector call
screens a batch; a sample whose 32 by 32 blocks all stay within a mean difference of 4 of the last
screened picture reuses that picture's regions.

Regions are followed from sample to sample in frame order: a match needs more than 0.45 box
overlap and an unchanged picture at the region's fixed anchor box, unique in both directions, and
a cut clears every match. Comparing at the anchor box rather than at each frame's own detector
box keeps static writing whole when the box jitters. A new region entered somewhere after the previous sample, and an unmatched one
left somewhere before this one. Bisection over the frames between them finds the first present or
first absent frame in at most ceil(log2(k)) probes; all searches of a batch advance together, one
screening call per step, with probes cached per frame. An occurrence keeps its entry frame and one
frame per matched sample; each frame ends where the next begins and the last ends at the first
absent frame, so the timing is exact. Document quads are in source pixels, scaled from the proxy.

After the stream, each occurrence's keyframe is its observed frame nearest the middle of its
interval. One full-resolution detection per keyframe still replaces that frame's quad by the best
overlapping region; an occurrence the server detector does not confirm is dropped as screening
noise, as is one shorter than 0.15 s or wider than half the frame. The still gives the occurrence's
surface colour for all its frames, its rectified crop in `visual/crops/` and its keyframe image,
at most 1280 pixels wide, in `visual/keyframes/`. Both folders are emptied when the keyframe phase
starts, so a rerun never leaves stale files behind. At most one million observations and one hundred
thousand occurrences are held; beyond that the scan fails and asks for shorter jobs.

## Boundaries

- Depends on: `media_io` frame streams, stills and proxy sizing, `inference::ocr::TextDetection`, `job_model` contracts, and the sibling `geometry` module.
- Used by: `pipeline::tasks::onscreen` for the scan and `tools/visual_validation` for `crop`.
- Rules: source videos are only read; no full-video image extraction; pending proxies stay within four samples with their gaps and stills four at a time; limits fail explicitly; every occurrence gets exactly one crop and one keyframe image.

## Related documentation

- [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md) — detection, review and presentation of visible writing.
- [Pipeline](/documentation/architecture/pipeline.md) — step order, workers and resume.
