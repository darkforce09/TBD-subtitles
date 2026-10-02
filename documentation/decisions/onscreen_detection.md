**Status:** live

# Decisions: on-screen text detection

The decisions about how the detection step screens frames for writing: at what resolution, how
many frames one detector call takes, the GPU memory it may hold, which PP-OCRv5 detector screens
and which confirms, and how the CUDA path picks its algorithms. "The previous entry" in the
entries of 2026-09-29 is the
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

### 2026-10-01 — The detector screens full-resolution frames, padded to a multiple of 32, on two sessions

**Context:** The proxy is 640 pixels wide, so small or faint writing that does not survive the
downscale is never found, and the owner wants it found. The detect-bench (commit 5febb2b,
`tools/visual_validation/src/detect_bench/`) measured on the RTX 3070 that screening full-resolution
frames with one detector session is bound by the GPU near 35 frames per second; that FFmpeg's
conversion to RGB caps decoding near 700 frames per second, while decoding to `yuv420p` reaches
about 1,500 and NVDEC about 770; and that the padded path held 1,472 MiB at batch 2 while batch 8
hit its 3 GiB arena cap. Each GPU worker may now hold 6.5 GB of VRAM
([decision](/documentation/decisions/foundations.md#2026-10-01--each-gpu-worker-stays-within-65-gb-of-vram-and-a-step-waits-up-to-a-deadline-for-the-memory-it-measured)).

**Decision:** `text_detect` decodes every frame at the source's resolution as 8-bit `yuv420p`
(10-bit sources included), through a pipe enlarged to 1 MiB, into a bounded queue of about four
seconds of frames whose buffers come from a recycled pool. The samples are the same as before:
every `round(fps / 2)`-th frame plus both frames around each cut, with no downscaling, and the
owner accepts the extra noise that brings. A sample is a duplicate when the 32 × 32 block means
of its Y plane differ from the last screened sample's by at most 4 on average; the rest are
converted in Rust to RGB with rows padded with black to a multiple of 32 (1,080 lines become
1,088), never stretched, and screened by the mobile PP-OCRv5 detector at a fixed input shape on
two sessions, each on its own thread and CUDA stream in the one worker process. The batch and the
session's arena limit are one measured pair, `ScreenShape { batch, pool_mib }`: it starts at
batch 4 per session, eight frames in flight, with the pool at the bench's padded batch-4 peak plus
headroom, and the host sweep of pool against batch fixes both before acceptance. A partial batch
is filled with black frames, so the shape never changes. Bisection probes are converted from the
held YUV frames and go ahead of the queued screening batches. Results are applied strictly in
sample order, so one and two sessions give the same document. Region signatures read the
full-range luma plane instead of a grey conversion of RGB, with the thresholds unchanged. The CUDA
provider runs with TF32, NHWC and a CUDA graph, its arena growing only as requested up to the pool;
a session that refuses the graph or NHWC reopens once without them, and the step's notes say so.
Decoding on the GPU (NVDEC) is a setting, off by default and outside every fingerprint, since
H.264 decoding is bit-exact. The detection step is at revision 5.

**Consequences:** Writing small enough to vanish in the proxy can be found, and the screen finds
more noise, which the existing drops (shorter than 0.15 s, wider than half the frame, unconfirmed
on its keyframe) and the owner's accepted limits absorb. The time of the step follows the GPU;
`text_detect`'s VRAM need is a named constant that the same sweep fixes within 6.5 GB, and the
step waits for that much free memory before it starts. The step's notes time warm-up, decode
wait, conversion, screening, probes, signatures and confirmation apart. Gap frames between two
samples are held as YUV until every transition that could land on them is resolved.

**Supersedes:** the proxy and batch-of-four parts, and the 3 GB arena, of 2026-09-29 — The
detector screens four proxies per call under a 3 GB arena; the proxy part of 2026-09-29 — The
mobile PP-OCRv5 detector screens proxies; the server detector confirms keyframes, whose choice of
detectors, anchor boxes, noise drops and tracking tolerance stand; and the 640-wide proxy, the
eight samples per call and "NVDEC is not needed" of 2026-09-29 — On-screen text is found by
sampled screening with bisected boundaries and read once per event by Claude vision.

### 2026-10-01 — The server detector confirms each occurrence at full resolution on the sample nearest its middle

**Context:** Each occurrence's keyframe was the observed frame nearest its midpoint, decoded again
at full resolution by an FFmpeg seek, four at a time, and confirmed by the server PP-OCRv5
detector at 960 × 544. Once the scan decodes every frame at full resolution, the screened samples
are already in memory, and the 24 GB of RAM leaves room to keep some of them.

**Decision:** The keyframe is the screened sample nearest the occurrence's middle. While an
occurrence is active, its window keeps the samples that can still be its middle, from
`(start + now) / 2 − step` to now, as shared YUV frames; a budget of 16 GiB bounds the candidates
held, and an occurrence whose candidate was let go falls back to an FFmpeg still. Confirmation
runs after the scan, so its order is fixed: the server detector, at full resolution with the same
padding, batch 1 and box score 0.5, on both sessions, each opening its confirmation session the
first time it needs one, after the screening sessions are dropped. Fallback stills decode eight at
once, and the next chunk is decoded while the current one is confirmed. Each confirmation
session's pool is a named constant the host sweep fixes.

**Consequences:** At 24 fps, with a 12-frame sample step, a keyframe can move by up to six frames
against the frame the earlier rule chose, so the crops, the 1280-wide still and the requests to
Claude change once. Most occurrences cost no FFmpeg seek; the step's notes count stills from RAM
against stills from FFmpeg. Peak VRAM is the larger of the screening and the confirmation phases,
never their sum.

**Supersedes:** the keyframe fetched by a seek of 2026-09-29 — On-screen text is found by sampled
screening with bisected boundaries and read once per event by Claude vision, and the keyframe pass
at 960 × 544 of 2026-09-29 — The detector screens four proxies per call under a 3 GB arena.

### 2026-10-01 — The CUDA path searches cuDNN algorithms exhaustively unless a repeat run disagrees

**Context:** The CUDA provider can search cuDNN's convolution algorithms exhaustively, with the
largest workspace, which is fastest but may pick different algorithms from one run to the next,
so two runs of `text_detect` could find slightly different boxes; or it can choose them by
heuristic with deterministic compute, which repeats exactly but runs slower. A TensorRT engine,
once built and cached, always runs the same kernels
([decision](/documentation/decisions/inference_engines.md#2026-10-01--tensorrt-runs-the-pp-ocrv5-detectors)).

**Decision:** The search is a named mode with two values: `Fast`, the exhaustive search with the
largest workspace, and `Deterministic`, the heuristic search with deterministic compute. `Fast` is
the default. The host check runs `text_detect` twice on one fresh job with the CUDA engine and
compares the two documents; if they differ, the default becomes `Deterministic`, and a new entry
records which mode holds and why.

**Consequences:** The detect-bench prints the boxes of both modes on the same frames. With the
TensorRT engine, runs that reuse its cached engine give identical results, while a rebuilt engine
may differ. The search runs during each session's warm-up, timed apart from screening.

**Supersedes:** none.

### 2026-10-01 — One server-detector session confirms

**Context:** The first host run of the detector pool's confirmation (Dressrosa 11, 40 stills,
CUDA, fast search) opened the server PP-OCRv5 detector on both session threads at 1920 × 1088,
batch 1. With two sessions every pool of 1,536 to 3,072 MiB failed: a single convolution asked
the arena for 558 MB, and at 3,072 MiB each the two sessions with the desktop's 1.3 GB no longer
fit on the 8 GB card. One session at 3,072 MiB confirmed 7.2 stills a second (138 ms each) with
the GPU 97 % busy and peaked at 3.3–3.6 GB of process VRAM; at 2,560 MiB it ran out of its pool.

**Decision:** One session confirms (`CONFIRM_SESSIONS`): the first session thread opens the
server detector once screening ends, and the other threads wait for the pool to close. The pool
options carry the confirming sessions and their memory pool apart from screening's, and the
detect-bench sweeps the confirmation pool (`--confirm-pools-mib`) so the host sets
`CONFIRM_POOL_MIB` from measurement.

**Consequences:** Confirmation takes about 0.14 s per occurrence on the RTX 3070 and holds one
session's memory; a second session would add no speed, since one already keeps the GPU busy.
Peak VRAM is still the larger of the two phases.

**Supersedes:** "on both sessions, each opening its confirmation session" in the entry of
2026-10-01 — The server detector confirms each occurrence at full resolution on the sample
nearest its middle; the rest of that entry holds.

### 2026-10-01 — Screening adds a 640-wide pass for writing too large at full resolution

**Context:** On Dressrosa 11 at 359 s the 海 painted on a wall is about 300 pixels tall. Full-
resolution screening missed it on every sample, on CUDA and on TensorRT alike, while the old
640-wide proxy found it: the mobile detector finds text at the sizes it was trained on, and a
glyph a third of the frame tall is beyond them until the frame is shrunk. The server detector
that confirms finds it at full resolution (a 324 × 271 box at score 0.86 on every still), so only
screening misses it.

**Decision:** Every screened sample, probes included, is screened twice in the same worker: at
full resolution and shrunk to 640 wide (`PROXY_WIDTH`, height in proportion), each proxy pixel the
mean of the source pixels it covers, on a second mobile session per screening thread
(`PROXY_POOL_MIB`). The proxy regions are scaled back to frame pixels and added only where the
full-resolution regions cover less than half of their bounding box. Confirmation stays at full
resolution. The owner chose this over training the detector.

**Consequences:** The 海 wall is found on CUDA and TensorRT; writing found at both sizes is not
doubled. Screening takes about 10 to 15 % longer (on the 海 clip, CUDA 37.3 to 33.9 frames a
second, TensorRT FP16 164 to 141) and each screening thread holds one more small session.

**Supersedes:** "there is no downscaled proxy" in the entry of 2026-10-01 — The detector screens
full-resolution frames, padded to a multiple of 32, on two sessions; the rest of that entry holds.

### 2026-10-02 — Two server-detector sessions confirm under TensorRT

**Context:** Previously, confirmation was restricted to one session (`CONFIRM_SESSIONS = 1`) because
two sessions under the ONNX Runtime CUDA provider arena failed to fit within the RTX 3070's 8 GB VRAM.
Under TensorRT, a batch-1 server detector session occupies only ~1.9 GB of VRAM.

**Decision:** Set `CONFIRM_SESSIONS = 2`. Both pool worker threads confirm keyframes concurrently on
TensorRT FP32 sessions once screening sessions close.

**Consequences:** Confirmation wall time is cut by ~50% (~10 s saved on a 30-minute episode). Peak VRAM
during confirmation is ~3.8 GB, safely below the 6.5 GB worker limit.

**Supersedes:** "One session confirms (`CONFIRM_SESSIONS`)" in the entry of
2026-10-01 — One server-detector session confirms; the rest of that entry holds.
