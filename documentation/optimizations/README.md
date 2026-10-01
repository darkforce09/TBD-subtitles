**Status:** live

# Optimizations

The designs behind milestones M6, M6.5, M7 and M8: using the owner's 32 GB machine, keeping decoded
frames where the next step needs them, missing fewer spoken lines and spelling names right, and
following moving writing while re-encoding only what changed. The owner picks each item; each
document says which items are built and what the host still measures before they are kept.

## Contents

```text
documentation/optimizations/
├── audio_accuracy.md        missed speech, names, batch context, separation and speaker turns
├── memory_profiles.md       the 24 GB target: the baseline, screening, decoding, overlap, models, frames
└── visual_and_video.md      homography, warped plates, partial re-encoding and smoothing
```

## How it works

Each document states what the code does today, read from the code and the measurements in
[research](/documentation/research/README.md), then one section per item: what changes, and what is
measured to keep it. Every item builds on the
[binary storage](/documentation/architecture/binary_storage_plan.md) that is in place: `job.redb`
per job with its per-frame `frames` and `readings` tables, and the sign library shared by episodes.

- [Memory profiles](memory_profiles.md) opens with the baseline that every target in the three
  milestones is stated against: each step's wall time, peak RAM, VRAM and GPU use on Dressrosa 11
  and 28 with the current build. Its items use the RAM headroom where it helps: full-resolution
  screening, faster decoding and encoding, overlapping steps that wait on Claude, a larger local
  translation model, and frames kept on the GPU, in one visual worker or in RAM between steps. It
  also lists the ideas the headroom does not help, and why.
- [Audio accuracy](audio_accuracy.md) recovers speech the backbone engine missed, gives Whisper
  the glossary as a prompt, learns name spellings across episodes in the sign library's file, adds
  context across adjudication batches, and measures separation ensembles, a third engine and
  speaker turns.
- [Visual and video](visual_and_video.md) follows moving writing with a homography per frame,
  inpaints a keyframe and warps it instead of inpainting every plate, re-encodes only the segments
  with replaced writing, and smooths the per-frame placements.

## Boundaries

- Depends on: [pipeline](/documentation/architecture/pipeline.md),
  [binary storage](/documentation/architecture/binary_storage_plan.md),
  [video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md) and the
  measurements in [research](/documentation/research/README.md).
- Used by: the [roadmap](/documentation/roadmap.md), milestones M6, M6.5, M7 and M8.
- Rules: documents stay within 500 lines; a number is a measurement with its source or a target
  stated against the baseline, never an estimate given as fact; no item invents dialogue or writes
  to a source video.

## Related documentation

- [Roadmap](/documentation/roadmap.md) — milestones M6, M6.5, M7 and M8.
- [Binary storage plan](/documentation/architecture/binary_storage_plan.md) — the redb and rkyv
  foundation.
- [Video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md) — the
  replacement steps as built.
- [The 24 GB decision](/documentation/decisions/foundations.md#2026-09-30--the-pipeline-targets-the-owners-32-gb-machine-24-gb-of-ram)
  — the memory limit; the VRAM cap is in the
  [6.5 GB decision](/documentation/decisions/foundations.md#2026-10-01--each-gpu-worker-stays-within-65-gb-of-vram-and-a-step-waits-up-to-a-deadline-for-the-memory-it-measured).
