**Status:** live

# Decisions: on-screen text detection

The decisions about how the detection step screens proxy frames for writing: how many one
detector call takes, the GPU memory it may hold, and which PP-OCRv5 detector screens and which
confirms. "The previous entry" in them is the
[sampled-screening entry](/documentation/decisions/stack_and_pipeline.md#2026-09-29--on-screen-text-is-found-by-sampled-screening-with-bisected-boundaries-and-read-once-per-event-by-claude-vision)
of the stack and pipeline decisions. The [decision log](/documentation/decisions/) says how
entries are written; the feature is
[Japanese on-screen text](/documentation/features/japanese_onscreen_text.md).

### 2026-09-29 — The detector screens four proxies per call under a 3 GB arena

**Context:** The first Dressrosa 11 run of the sampled scan screened a batch of four 640-wide
proxies and then failed on the next call: ONNX Runtime's CUDA arena, capped at 2 GB, had 203 MB
left when `Conv.130` of the server PP-OCRv5 detector asked for 642 MB for a batch of eight. The
server detector keeps roughly 450 MB of activations per proxy, so eight per call cannot fit in
2 GB, and the previous entry's "eight samples per predictor call" was never measured.

**Decision:** The screening batch is four proxies, and every bisection probe call is split to
four as well; the OCR worker's CUDA arena limit is 3 GB, which with the CUDA context, the
detector's 88 MB of weights and the bounded convolution workspace stays under the 5.5 GB worker
budget. The pending samples and their gaps shrink accordingly to four samples.

**Consequences:** One screening call holds at most about 1.8 GB of activations, with headroom
for the full-resolution keyframe pass at 960×544. Throughput is measured on the same episode
with this batch size rather than assumed from the larger one.

**Supersedes:** the batch size named in the previous entry; the rest of it stands.

### 2026-09-29 — The mobile PP-OCRv5 detector screens proxies; the server detector confirms keyframes

**Context:** With four proxies per call, the server PP-OCRv5 detector screened Dressrosa 11 at
about 85 frames of video per second: the GPU sat at 88 % while FFmpeg used less than one core,
so the detector, not decoding, bounded the scan at roughly nine minutes for 44,489 frames
against the six-minute budget for 50,000. The brief's 7–10 ms per proxy assumed a lighter
network than the 88 MB server export.

**Decision:** Screening and bisection probes run the mobile PP-OCRv5 detector,
`pp-ocrv5_mobile_det.onnx` from the same pinned oar-ocr release (4.8 MB), at the 0.3 box score;
the server detector keeps the keyframe pass at 0.5, so every occurrence's geometry, crop and
surface colour still come from the stronger model. The same run showed 2,379 occurrences, most
of them one animation drawing long, because the detector's box jitters around static writing and
the anchor signature was cropped at each frame's own box: a 49-pixel and a 75-pixel box of the
same credits line never matched. Every comparison now crops the current proxy at the region's
fixed anchor box, and the scan drops writing shorter than 0.15 s (one drawing on animation held
on twos or threes), wider than half the frame, or unconfirmed by the server detector on its
keyframe. Both exports are downloaded, never converted,
and the detection step's fingerprint covers both files. Tracking accepts a surface as static when
every sampled box keeps its centre within a fifth of the keyframe box height and half its overlap,
since the detector's box grows and shrinks around unmoved writing by tens of pixels; a two-pixel
corner tolerance flagged 136 of 143 occurrences on this episode.

**Consequences:** Presence screening trades some recall on faint or tiny writing for speed,
inside the misses the owner already accepts, and a sign shown for fewer than four frames at
24 fps counts as noise; the synthetic scenario's brief sign lasts four frames for that reason. The pp-ocrv5 model folder gains
one file, which Settings offers to download.

**Supersedes:** none.
