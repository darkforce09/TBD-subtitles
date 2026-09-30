**Status:** live

# Optimizations

The technical designs and roadmap for advancing subtitle accuracy, visual stability, video
re-encoding throughput, and memory scaling across hardware profiles.

## Contents

```text
documentation/optimizations/
├── audio_accuracy.md        dialogue completeness, Whisper prompt biasing, orphan recovery and voting
├── memory_profiles.md       direct 24 GB workstation architecture: DDR5 bandwidth, 44.1 kHz audio, 1080p video
└── visual_and_video.md      homography perspective tracking, temporal inpaint warping and fast re-encoding
```

## How it works

The optimizations build upon the binary storage architecture (`redb` and `rkyv`). With
step outputs and frame records stored in high-performance binary tables, the pipeline can
perform sub-millisecond range lookups across 400,000+ frames and audio timelines.

- [Audio accuracy](audio_accuracy.md) addresses the remaining 5% of speech recognition errors:
  orphan recovery in the diff sheet, dynamic arc prompt biasing in Whisper, cross-episode
  glossary learning via `library.redb`, and multi-engine acoustic voting.
- [Visual and video](visual_and_video.md) adds temporal stability to on-screen text: planar
  homography for moving signs, motion-warped inpainting to eliminate background flicker, and
  GOP-aligned lossless segment re-encoding that cuts `localized_video` runtime from minutes to seconds.
- [Memory profiles](memory_profiles.md) details the 24 GB target architecture tailored for the
  owner's 32 GB DDR5-6000 workstation, enabling concurrent audio/visual execution, studio-quality 44.1 kHz
  vocal separation, native 1080p screening, uncompressed video ring buffers, and global diarization.

## Boundaries

- Depends on: [`pipeline.md`](/documentation/architecture/pipeline.md),
  [`binary_storage_plan.md`](/documentation/architecture/binary_storage_plan.md), and
  [`job_model`](/crates/job_model/README.md).
- Used by: the roadmap milestones M6, M7 and M8, the job runner, and the desktop GUI settings.
- Rules: documents stay at or under 500 lines; all milestones and features preserve the invariant
  that no dialogue is invented and video source files remain read-only.

## Related documentation

- [Roadmap](/documentation/roadmap.md) — milestones M6, M7, and M8.
- [Binary storage plan](/documentation/architecture/binary_storage_plan.md) — the redb and rkyv foundation.
- [Video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md) — baseline inpainting.
