**Status:** live

# Workstation memory architecture: the 24 GB target

How the pipeline uses the owner's machine: an i7-14700K (28 threads), 32 GB of DDR5-6000 and an
RTX 3070 with 8 GB. Peak RAM stays within **24 GB**, leaving 8 GB to the desktop
([decision](/documentation/decisions/foundations.md#2026-09-30--the-pipeline-targets-the-owners-32-gb-machine-24-gb-of-ram)),
and each GPU worker within **6.5 GB of VRAM**, each GPU step waiting, up to a deadline, for the
memory it measured
([decision](/documentation/decisions/foundations.md#2026-10-01--each-gpu-worker-stays-within-65-gb-of-vram-and-a-step-waits-up-to-a-deadline-for-the-memory-it-measured)).
These are the throughput items of milestones M6 and M6.5; each is built only when the owner picks
it, and each is kept only when a measurement on a real episode shows it pays.

## Target the actual machine

The 8 GB limit kept the first milestones lean. The app runs on one machine, so it is designed for
that machine directly, with no stepping stones at 8 or 16 GB. Two things do not change:

- **VRAM decides what fits on the card.** The detector, LaMa, the speech models and the local
  translation model are limited by the GPU, not by RAM; extra RAM buys them nothing except
  larger batches and room to hold frames.
- **One GPU step at a time.** GPU steps run in their own worker processes under the machine-wide
  GPU lock ([pipeline](/documentation/architecture/pipeline.md)), so two GPU steps never share the
  card. Steps can overlap only where one of them waits on something other than the GPU.

```text
[32 GB host]
├── desktop, display server, browser   about 8 GB
└── TBD-subtitles, every process        24 GB RAM at peak; each GPU worker 6.5 GB VRAM at most
```

**The memory wait.** Each GPU step has a VRAM need: its measured peak in the
[M6 baseline](/documentation/research/m6_baseline.md) plus 256 MiB, never the cap itself;
`text_detect`'s is a named constant that the host sweep of the
[runbook](/documentation/runbooks/measuring_full_resolution_screening.md) fixes. After taking the
GPU lock, the worker polls NVML's free memory every second until it covers the need, telling the
window "waiting for GPU memory: N MiB free, M needed"; it honours cancel, and after ten minutes it
fails the step with the free memory, the need and "close other GPU programs and retry". Without
NVML there is no check. Built, awaiting the host measurement.

The [binary storage](/documentation/architecture/binary_storage_plan.md) that these items build on
is in place: `job.redb` per job with its per-frame `frames` and `readings` tables, the worker
channel, and the sign library shared by episodes.

## 1. Baseline first

Every target in M6, M7 and M8 is stated against one baseline, measured before any item is built:
Dressrosa 11 and 28 run from scratch (`process --no-library`, a fresh work root) on the host.
Beside each step's wall time, load time, peak RAM, peak child RAM and peak VRAM, the job report
gives what the baseline needs:

- the step's speed against the video (× real time) and the frame rates of the frame steps;
- the CPU cores busy and the busiest thread, and how busy the GPU, NVENC and NVDEC are, sampled
  every 250 ms, so idle stretches and single-thread bounds are known rather than guessed;
- the localized video's time split into decode wait, blend, encode wait and flush, and the
  screen's into decode wait, detection, confirmation and stills;
- the whole job's peak RAM across all its processes at once, and the run's real wall time.

The [M6 baseline](/documentation/research/m6_baseline.md) records it: Dressrosa 11 and 28 take
16.4 and 15.1 minutes of real wall time; `localized_video` (encoder-bound, NVENC at 100 %),
`text_detect` (detection 66 % of it with the GPU 27 % busy) and `adjudicate` (waiting on Claude,
GPU and CPU idle) take two thirds of it; whole-job peak RAM is 3.9 GB; the separation worker
holds 7.3 GB of VRAM, over the per-worker cap.

The items marked built below change the job database's layout, so every existing job runs all its
steps once more. They are compared on fresh runs of Dressrosa 11 and 28, with the visual and
localized steps judged on their own outputs, since Claude's answers differ between runs from
scratch ([runbook](/documentation/runbooks/measuring_full_resolution_screening.md)).

## 2. Full-resolution visual screening

Built, awaiting the host measurement
([decision](/documentation/decisions/onscreen_detection.md#2026-10-01--the-detector-screens-full-resolution-frames-padded-to-a-multiple-of-32-on-two-sessions)).
Detection screens the frames at the source's resolution, with no downscaled proxy, so small or
faint writing a downscale would lose can be found; the owner accepts the extra noise.

- **Frames:** FFmpeg decodes every frame at full resolution as 8-bit `yuv420p` through a pipe
  enlarged to 1 MiB; a decode thread fills a bounded queue of about four seconds of frames from a
  recycled buffer pool, so FFmpeg never waits on the detector and no frame is allocated anew.
  Gap frames between samples stay as YUV until every transition that could land on them is
  resolved.
- **Screening:** the samples are unchanged (every `round(fps / 2)`-th frame and both frames around
  each cut). A sample whose Y-plane 32 × 32 block means differ from the last screened sample's by
  at most 4 reuses its detections; the others are converted in Rust with rayon to RGB padded with
  black to 1,088 lines and screened by the mobile PP-OCRv5 detector on two sessions, each on its
  own thread and CUDA stream in the one worker. Batch and arena pool are one measured pair,
  starting at batch 4 per session. Bisection probes are converted from held YUV and go ahead of
  the queued batches; results apply in sample order, so one and two sessions give one document.
- **Keyframes from RAM:** the keyframe is the screened sample nearest the occurrence's middle,
  held as YUV within a 4 GiB budget; the server detector confirms it at full resolution on both
  sessions after the scan, and an evicted candidate falls back to an FFmpeg still, eight decoding
  at once
  ([decision](/documentation/decisions/onscreen_detection.md#2026-10-01--the-server-detector-confirms-each-occurrence-at-full-resolution-on-the-sample-nearest-its-middle)).
- **Detector engine:** CUDA by default, with TF32, NHWC and a CUDA graph, searching cuDNN's
  algorithms exhaustively
  ([decision](/documentation/decisions/onscreen_detection.md#2026-10-01--the-cuda-path-searches-cudnn-algorithms-exhaustively-unless-a-repeat-run-disagrees));
  or TensorRT FP16 engines built once and cached, a setting until the host bench confirms it
  ([decision](/documentation/decisions/inference_engines.md#2026-10-01--tensorrt-runs-the-pp-ocrv5-detectors)).
- **What the host measures:** the pool × batch sweep that fixes the batch, the screening and
  confirmation pools and `text_detect`'s VRAM need within 6.5 GB; the overlap of the two sessions;
  the determinism of two runs; occurrences found against the baseline, detection wall time, peak
  VRAM of the detector worker and peak RAM.

## 3. Faster decoding and encoding

- **Hardware decoding.** Built, awaiting the host measurement: Settings, On-screen Text, "Decode
  video on the GPU (NVDEC)" lets the screen's stream decode on NVDEC into nv12 frames; it is off
  (the CPU) by default and outside every fingerprint, since H.264 decoding is bit-exact. The
  detect-bench measured NVDEC at about 770 frames per second against about 1,500 for the CPU's
  `yuv420p` on the RTX 3070, so the CPU stays the default unless the host run shows otherwise.
- **YUV regions for the replacement steps.** Built, awaiting the host measurement: `text_mask`,
  `text_inpaint` and `text_verify` ask FFmpeg for `yuv420p` cropped to the even-aligned rectangle
  around the region, rather than RGB, and Rust trims and converts it with the stream's matrix and
  range; the decode runs ahead on a thread through a bounded queue with pooled buffers.
- **Overlapped localized video.** Built, awaiting the host measurement: `localized_video` runs a
  decode thread, the blend on the step's thread and an encoder thread that owns the FFmpeg
  encoder, joined by bounded queues (about four seconds of decoded frames, eight blended ones),
  so each runs ahead of the next; the phase notes are measured per thread.
- **Measured:** decode, blend and encode time of `localized_video`, frames per second of the
  screen, and the replacement steps' times against the baseline. Re-encoding only what changed is
  the M8 item in [visual and video](visual_and_video.md#3-re-encoding-only-what-changed).

## 4. Overlapping steps that wait on different things

Steps run in `StepName::ALL` order on the runner's main walk, with two lanes beside it. The shot
scan runs beside the steps after it until the visual lane starts. `adjudicate`, `readjudicate`
and `sound_cues` wait on Claude and leave the GPU and the CPU idle; `text_detect`, `text_read`
and `text_track` need only the probe and the shot scan, so they run as the visual lane, on a
thread of their own, beside adjudication and the audio steps after it.

- **How it runs:** the main walk starts the lane when it reaches `adjudicate`
  (`graph::VISUAL_LANE_STARTS_AT`), after the shot scan has finished, and waits for it before
  `text_translate`, the first step that reads it. Everything stays within the GPU lock (two GPU
  steps, the lane's and the main walk's, still never overlap; the one waiting names the holder)
  and within 24 GB.
- **Audio first:** while an audio step waits for the GPU lock, a visual step stays off it, so the
  audio steps on the main walk never queue behind the lane. Built, awaiting the host measurement.
- **The lock for `text_translate`:** the step waits on Claude for most of its time, so it takes
  the GPU lock and the memory wait only just before the local model loads, and holds them until
  the model is dropped; the wait's message reaches the window through the worker channel. Built,
  awaiting the host measurement.
- **Measured:** whole-job wall time, peak RAM of all processes together and VRAM per worker on
  Dressrosa 11 and 28 against the baseline; whether `text_detect` now ends before adjudication;
  the subtitle files and approved replacements stay the same.

## 5. A larger local translation model

The local model translates only the on-screen writing Claude leaves unanswered; on Dressrosa 11
that was two occurrences. It is Qwen3.5-4B at 4-bit (2.7 GB of weights) in its own mistral.rs
worker.

- **The trial:** a 7B-class model at 4-bit, about 4.5 GB of weights, which fits within the 6.5 GB
  VRAM cap with its context.
- **Not on the card:** a 14B model at 4-bit is about 8 to 9 GB, over the cap; it runs only with
  part of it offloaded to the CPU and its RAM, and is tried only if the 7B trial shows larger
  models translate better.
- **Measured:** translation quality on the occurrences Claude leaves and with Claude turned off,
  load and translation time, peak VRAM and RAM, against Qwen3.5-4B.

## 6. Frames kept where the next step needs them

The items of milestone M6.5. None is built; each waits for the measurement it names.

- **GPU decoding with frames kept on the GPU.** NVDEC, through the Video Codec SDK and a Rust
  crate, decodes into device memory; YUV→RGB, padding and normalisation run as CUDA kernels and
  feed the TensorRT detector without a host round trip. It needs law 3 amended; FFmpeg stays for
  muxing, encoding, probing and the fallback. It is built only if the TensorRT measurement shows
  decoding, conversion or host↔device copies dominating `text_detect`; on the RTX 3070, NVDEC
  measured about 770 frames per second against about 1,500 for the CPU's `yuv420p`
  (detect-bench, commit 5febb2b).
- **One ONNX Runtime GPU process across the visual steps.** `text_detect`, `text_read`,
  `text_mask`, `text_inpaint` and `text_verify` run in one long-lived worker that keeps their
  sessions loaded and a bounded frame cache, instead of one process per step. ggml and mistral.rs
  keep their own processes. It needs law 7 amended. The measured ceiling comes first: the summed
  `load_s` and decode time of those steps on Dressrosa 11 and 28.
- **Shared frame cache in RAM.** `text_detect` writes the decoded YUV frames of each occurrence's
  span, widened by the margins later steps read, to a memory-mapped file under `/dev/shm`, within
  a fixed budget (for example 4 GiB, part of the 24 GB); `text_mask`, `text_inpaint` and
  `text_verify` read frames from it instead of starting FFmpeg seeks, and fall back to FFmpeg for
  frames outside it. The cache is per job, removed when the job ends or is reopened, and never
  part of a fingerprint, since decoded frames are bit-exact. It needs no change to law 7.
  Measured: those steps' decode time on Dressrosa 11 and 28, before and after.

## What the headroom does not buy

These ideas came up and are left out, each for a reason in the code:

- **A shared mel-spectrogram cache.** Voice detection, sound events, alignment and the speech
  engines each compute their own features with their own settings inside their own worker
  process (law 7); the features are cheap next to the models, and one shared tensor would have
  to cross process boundaries.
- **A resident cache of plates, masks and patches.** `localized_video` time is decoding and
  encoding, not disk reads; its patches are already held in memory within 512 MiB, and the
  plate, mask and patch folders of Dressrosa 11 take tens of megabytes.
- **Separation at 44.1 kHz for the speech engines.** Separation already reads 44.1 kHz stereo;
  it writes 16 kHz stems because Parakeet, Whisper, the aligner and the sound-event model take
  16 kHz input only, so no engine hears above 8 kHz whatever the RAM.
- **Separation windows of several minutes.** The exported separation models take a fixed window
  (11 s for Mel-Band RoFormer); the overlap-add already weights every window.

## Boundaries

- Depends on: [pipeline](/documentation/architecture/pipeline.md),
  [binary storage](/documentation/architecture/binary_storage_plan.md), and the owner's 32 GB
  host.
- Used by: roadmap milestones M6 and M6.5, and the
  [host runbook](/documentation/runbooks/measuring_full_resolution_screening.md).
- Rules: peak RAM within 24 GB; each GPU worker within 6.5 GB of VRAM; memory bounded, never grown
  with the video; source videos read-only; an item is kept only when its measurement against the
  baseline shows it pays.

## Related documentation

- [Roadmap](/documentation/roadmap.md#m6--24-gb-workstation-throughput) — milestone M6, and
  [M6.5](/documentation/roadmap.md#m65--gpu-resident-frames-and-a-shared-visual-worker).
- [Measuring full-resolution screening](/documentation/runbooks/measuring_full_resolution_screening.md)
  — the host steps that measure the built items.
- [Audio accuracy](audio_accuracy.md) — separation ensembles and the speech engines.
- [Visual and video](visual_and_video.md) — following moving writing and re-encoding only what
  changed.
