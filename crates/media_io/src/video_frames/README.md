# Video frame stream

Video frames with their presentation timestamps, read through bounded FFmpeg and ffprobe runs:
RGB frames scaled to a size, every frame at its native size in an encoder's pixel format, single
frames at a presentation time, and runs of frames cropped to one region.

## Contents

```text
crates/media_io/src/video_frames/
├── mod.rs      the decode mode, the frame stream and its rgb24 constructor, the frame reads
├── native.rs   `PixelFormat` and `FrameStream::open_native`: unscaled frames in a raw format
├── packets.rs  `timeline`: the presentation timeline from the container origin and packet table
├── region.rs   `RegionStream`: consecutive full-resolution frames cropped to one rectangle
├── still.rs    one frame at a presentation time, through a half-frame-early accurate seek
└── tests/      packet timing, frame and timestamp agreement, native sizes, stills and regions
```

## How it works

`timeline` knows every frame's time before anything decodes. A bounded ffprobe reads the
container's start time, the origin of every timestamp, which preserves the video's offset from the
audio. A second ffprobe lists the first video stream's packets, which the demuxer reads without
decoding. A packet flagged for the decoder to discard, such as the pre-roll an MP4 edit list skips,
and a packet without a presentation time have no frame; the rest are sorted into presentation
order, because B-frame packets are stored in decode order. Each frame ends where the next begins,
and the last after its reported duration or `1 / fps`; a repeated or reversed time is an error.
`FrameStream::timeline()` hands the whole table out before the first frame, and callers that
decode regions read it directly.

`FrameStream::open` then starts one FFmpeg decoder that streams raw rgb24 frames scaled to a size
with no frame-rate conversion, and `next_frame` pairs the frame at each index with the timeline
entry at that index. `FrameStream::open_native` streams the same frames unscaled in a
`PixelFormat`: rgb24, or 8- or 10-bit 4:2:0 for the encoder, whose byte size per frame
`PixelFormat::frame_bytes` computes and which needs an even width and height. The count must
match exactly: a partial frame, a frame past the timeline or an end before it is an error, and
`finish` fails unless the decoder exited cleanly after one frame per entry. A caller that stops
early drops the stream, which stops the decoder. `Decode::Proxy` has the decoder skip its in-loop
deblocking filter, which is faster and fit only for screening; `Decode::Exact` decodes as encoded.

`still::still` decodes the one frame at an origin-relative time, scaled like the stream. It seeks
half a frame early: FFmpeg's accurate seek drops every frame that presents before the seek point,
so the first frame left is the one at that time, byte for byte the frame the stream yields.
Anything but one full frame is an error.

`region::RegionStream` seeks the same way and decodes `count` consecutive frames at full
resolution, each converted to rgb24 at full size and then cropped, so a crop at odd coordinates
equals the same rectangle of a full-size rgb24 frame. A caller that knows the gap to the previous
frame passes its inverse as the rate, so the half-frame margin holds at a variable rate. The crop
must lie inside the frame (FFmpeg moves an overflowing crop back inside); `finish` fails unless
exactly `count` crops arrived and FFmpeg exited cleanly. Its deadline grows with `count`.

## Boundaries

- Depends on: `child_process` and the parent media types.
- Used by: the visible-text stages (detection, and region crops for replacement), the
  localized-video encoder, and the validation harness.
- Rules: source videos are read-only; frame buffers have a checked size; no decoder is linked.

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — stage flow and worker ownership.
- [Video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md) — the
  region crops and native frames the replacement steps read.
