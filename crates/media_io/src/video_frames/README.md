# Video frame stream

RGB frames with their presentation timestamps, read through bounded FFmpeg and ffprobe runs, and
single frames at a presentation time.

## Contents

```text
crates/media_io/src/video_frames/
├── mod.rs    the decode mode, the packet timeline and the RGB frame stream that must match it
├── still.rs  one frame at a presentation time, through a half-frame-early accurate seek
└── tests/    packet timing, frame and timestamp agreement, and stills against streamed frames
```

## How it works

`FrameStream::open` knows every frame's time before it decodes one. A bounded ffprobe reads the
container's start time, the origin of every timestamp, which preserves the video's offset from the
audio. A second ffprobe lists the first video stream's packets, which the demuxer reads without
decoding. A packet flagged for the decoder to discard, such as the pre-roll an MP4 edit list skips,
and a packet without a presentation time have no frame; the rest are sorted into presentation
order, because B-frame packets are stored in decode order. Each frame ends where the next begins,
and the last after its reported duration or `1 / fps`; a repeated or reversed time is an error.
`timeline()` hands the whole table out before the first frame.

One FFmpeg decoder then streams raw rgb24 frames with no frame-rate conversion, and `next_frame`
pairs the frame at each index with the timeline entry at that index. The count must match exactly:
a partial frame, a frame past the timeline or an end before it is an error, and `finish` fails
unless the decoder exited cleanly after one frame per entry. A caller that stops early drops the
stream, which stops the decoder. `Decode::Proxy` has the decoder skip its in-loop deblocking
filter, which is faster and fit only for screening; `Decode::Exact` decodes as encoded.

`still::still` decodes the one frame at an origin-relative time, scaled like the stream. It seeks
half a frame early: FFmpeg's accurate seek drops every frame that presents before the seek point,
so the first frame left is the one at that time, byte for byte the frame the stream yields.
Anything but one full frame is an error.

## Boundaries

- Depends on: `child_process` and the parent media types.
- Used by: the visible-text stages and their validation harness.
- Rules: source videos are read-only; frame buffers have a checked size; no decoder is linked.

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — stage flow and worker ownership.
