# Video frame stream

RGB frames and their presentation timestamps, read through bounded FFmpeg and ffprobe pipes.

## Contents

```text
crates/media_io/src/video_frames/
├── mod.rs  child lifetime, RGB frames and timestamp pairing
└── tests/  variable-duration timestamp parsing
```

## How it works

`FrameStream` reads one frame and its matching timestamp at a time. It preserves source cadence,
normalizes against the common container origin and uses the next presentation timestamp for
variable-rate frame ends. A bounded header probe preserves audio/video offsets. Incomplete
output is an error. Dropping the stream stops both children.

## Boundaries

- Depends on: `child_process` and the parent media types.
- Used by: the visible-text stages and their validation harness.
- Rules: source videos are read-only; frame buffers have a checked size; no decoder is linked.

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — stage flow and worker ownership.
