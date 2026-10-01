**Status:** live

# Workstation memory architecture: the 24 GB target

How the pipeline uses the owner's machine: an i7-14700K (28 threads), 32 GB of DDR5-6000 and an
RTX 3070 with 8 GB. Peak RAM stays within **24 GB**, leaving 8 GB to the desktop, and each GPU
worker within **5.5 GB of VRAM**
([decision](/documentation/decisions/foundations.md#2026-09-30--the-pipeline-targets-the-owners-32-gb-machine-24-gb-of-ram)).
These are the throughput items of milestone M6; each is built only when the owner picks it, and
each is kept only when a measurement on a real episode shows it pays.

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
└── TBD-subtitles, every process        24 GB RAM at peak; each GPU worker 5.5 GB VRAM at most
```

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
holds 7.3 GB of VRAM, over the 5.5 GB cap.

## 2. Full-resolution visual screening

`text_detect` screens a proxy 360 lines high (640 pixels wide for 16:9) decoded with the loop
filter skipped: about two samples per second, both frames around every shot cut, and the frames
bisection probes between samples ([text detection](/crates/stages/src/onscreen_text/detect/README.md)).
Only the keyframe of each occurrence is detected again at full resolution. Small or faint
writing that does not survive the downscale is never found; the owner wants it found.

- **What changes:** samples and bisection probes are screened at the source resolution. The
  mobile PP-OCRv5 detector runs on the GPU through CUDA, so its speed and the batch it can take
  are bounded by GPU throughput and VRAM, not by CPU threads. RAM holds full-resolution frames
  between samples (about 6 MB per 1080p RGB frame) and allows larger batches.
- **What it costs:** a frame of six times the pixels per detector call, and a decoder that now
  delivers full-size frames; on Dressrosa 11 the proxy decoder was already the busiest process
  ([visual scan](/documentation/research/visual_scan_dressrosa_11.md)).
- **Measured:** occurrences found at full resolution against the proxy on Dressrosa 11 and 28,
  detection wall time, peak VRAM of the detector worker and peak RAM, against the baseline.

## 3. Faster decoding and encoding

- **Hardware decoding.** Frames for the screen and the localized video are decoded by FFmpeg on
  the CPU today; only the shot scan can ask for NVDEC, and runs without it, because its frames
  are scaled on the CPU either way. NVDEC with the frames downloaded for Rust, or scaled on the
  GPU for the proxy, is measured against the CPU decoder on the host.
- **Overlapped localized video.** `localized_video` runs three processes in a chain: the FFmpeg
  decoder, the blend, and the FFmpeg encoder, joined by pipes far smaller than one frame. A
  bounded queue of frames between them lets each run ahead of the next; its size is set from
  the measured stalls and stays bounded.
- **Measured:** decode, blend and encode time of `localized_video` and frames per second of the
  screen against the baseline. Re-encoding only what changed is the M8 item in
  [visual and video](visual_and_video.md#3-re-encoding-only-what-changed).

## 4. Overlapping steps that wait on different things

Steps run in `StepName::ALL` order on the runner's main walk, with two lanes beside it. The shot
scan runs beside the steps after it until the visual lane starts. `adjudicate`, `readjudicate`
and `sound_cues` wait on Claude and leave the GPU and the CPU idle; `text_detect`, `text_read`
and `text_track` need only the probe and the shot scan, so they run as the visual lane, on a
thread of their own, beside adjudication and the audio steps after it. `text_translate` still
waits on Claude for most of its time while it holds the GPU lock in case the local model loads.

- **How it runs:** the main walk starts the lane when it reaches `adjudicate`
  (`graph::VISUAL_LANE_STARTS_AT`), after the shot scan has finished, and waits for it before
  `text_translate`, the first step that reads it. Everything stays within the GPU lock (two GPU
  steps, the lane's and the main walk's, still never overlap; the one waiting names the holder)
  and within 24 GB.
- **Measured:** whole-job wall time, peak RAM of all processes together and VRAM per worker on
  Dressrosa 11 and 28 against the baseline; the subtitle files and approved replacements stay
  the same.

## 5. A larger local translation model

The local model translates only the on-screen writing Claude leaves unanswered; on Dressrosa 11
that was two occurrences. It is Qwen3.5-4B at 4-bit (2.7 GB of weights) in its own mistral.rs
worker.

- **The trial:** a 7B-class model at 4-bit, about 4.5 GB of weights, which fits within the 5.5 GB
  VRAM cap with its context.
- **Not on the card:** a 14B model at 4-bit is about 8 to 9 GB, over the cap; it runs only with
  part of it offloaded to the CPU and its RAM, and is tried only if the 7B trial shows larger
  models translate better.
- **Measured:** translation quality on the occurrences Claude leaves and with Claude turned off,
  load and translation time, peak VRAM and RAM, against Qwen3.5-4B.

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
- Used by: roadmap milestone M6.
- Rules: peak RAM within 24 GB; each GPU worker within 5.5 GB of VRAM; memory bounded, never grown
  with the video; source videos read-only; an item is kept only when its measurement against the
  baseline shows it pays.

## Related documentation

- [Roadmap](/documentation/roadmap.md#m6--24-gb-workstation-throughput) — milestone M6.
- [Audio accuracy](audio_accuracy.md) — separation ensembles and the speech engines.
- [Visual and video](visual_and_video.md) — following moving writing and re-encoding only what
  changed.
