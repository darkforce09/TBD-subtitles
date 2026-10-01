# Video frame stream

Video frames with their presentation timestamps, read through bounded FFmpeg and ffprobe runs:
RGB frames scaled to a size, every frame at its native size in an encoder's pixel format,
full-resolution 8-bit YUV frames in recycled buffers, single frames at a presentation time, and
runs of frames cropped to one region.

## Contents

```text
crates/media_io/src/video_frames/
├── mod.rs         the decode mode, the rgb24 frame stream and the frame reads
├── native.rs      `PixelFormat` and `FrameStream::open_native`: unscaled raw frames
├── packets.rs     `timeline` and `keyframes`: frame times and keyframe frames from the packets
├── pipe.rs        FFmpeg's stdout pipe enlarged to 1 MiB, never past `pipe-max-size`
├── region.rs      `RegionStream`: frames cropped to one rectangle, rgb24 in the stream's colour
├── still.rs       one frame at a presentation time, through a half-frame-early seek
├── yuv_stream.rs  `YuvStream`: full-size yuv420p or NVDEC nv12 frames from any index, pooled
└── tests/         timing, frame agreement, native sizes, YUV starts, pipes, stills and regions
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

`YuvStream` is the full-resolution stream for screening, replacement and the localized video. It
asks FFmpeg for 8-bit `yuv420p` whatever the source's depth, or with `YuvOptions::hardware`
decodes through NVDEC (`-hwaccel cuda`) and downloads nv12, which suits 8-bit 4:2:0 sources
(`hardware_suits`). Nothing skips a decoding step. `YuvOptions::first` starts at any frame index:
the seek lands half the gap to the previous frame before that frame's time, so FFmpeg's accurate
seek drops everything earlier and the first frame out is exactly that index, byte for byte the
frame a run from the start yields; a segment can therefore be decoded from its keyframe.
`YuvOptions::count` stops after that many frames and `YuvOptions::crop` has FFmpeg crop each
frame to an even-aligned rectangle. `pipe::enlarge` raises the stdout pipe to 1 MiB, never past
`/proc/sys/fs/pipe-max-size`, so a frame crosses in few reads. Each frame is read into a buffer
from a `BufferPool` and handed out as a `yuv::YuvFrame` with its index and times from the
timeline; dropping the frame returns the buffer, so a warm stream allocates nothing per frame.
`YuvStream` is a `frame_queue::Producer`: `spawn` runs it on a decode thread through a
`FrameQueue` about four seconds of frames deep (at most 512 MiB). The checks are
`FrameStream`'s: a missing, partial or surplus frame is an error, a decoder that ends early
reports its own exit, and `finish` fails unless FFmpeg exited cleanly after every frame asked for.

`still::still` decodes the one frame at an origin-relative time, scaled like the stream. It seeks
half a frame early: FFmpeg's accurate seek drops every frame that presents before the seek point,
so the first frame left is the one at that time, byte for byte the frame the stream yields.
Anything but one full frame is an error.

`region::RegionStream` decodes `count` consecutive frames from a frame index through a
`YuvStream` cropped to `RegionCrop::around` the region: the left and top edges rounded down to
even and the right and bottom up to even, within the frame, so FFmpeg splits no chroma sample.
Each `yuv420p` crop is trimmed back to the region and converted to rgb24 in Rust with the
stream's matrix and range (`yuv::crop_to_rgb` with `Coefficients::of`), so a crop at odd
coordinates equals the same rectangle of the whole frame converted alike. Against FFmpeg's own
rgb24 conversion of an untagged test clip, 99.9 % of samples differ by at most two levels and
the largest difference is three. `RegionStream` is a `Producer` too: `spawn` converts the crops
on the decode thread, ahead of the reader. A region outside the frame or a run past the timeline
is refused before FFmpeg starts; `finish` fails unless exactly `count` crops arrived and FFmpeg
exited cleanly. Its deadline grows with `count`.

## Boundaries

- Depends on: `child_process`, `libc` for the pipe size, the parent media types, `yuv` for the
  frames and their conversion, and `frame_queue` for the buffer pool and the decode thread.
- Used by: the on-screen text stages in `crates/stages/src/onscreen_text/` (the detection stream
  and stills, and region crops for replacement), the localize stage in
  `crates/stages/src/localize/` (native frames for the encoder) and the detection benchmark in
  `tools/visual_validation/` (the pipe size).
- Rules: source videos are read-only; frame buffers have a checked size; no decoder is linked; a
  start at a frame index yields that frame first
  (`a_start_at_any_index_yields_exactly_that_frame_first`); the wide pipe never exceeds 1 MiB or
  `pipe-max-size` (`the_wide_pipe_never_exceeds_one_mebibyte_or_the_system_limit`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — stage flow and worker ownership.
- [Video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md) — the
  region crops and native frames the replacement steps read.
