# Media input source

The `media_io` library: one module folder for each thing the pipeline reads from a video, which
are the probe result, the decoded audio, the shot-change times and the video frames, the encode
that writes the localized video, and the crate's `Programs` and `MediaError`.

## Contents

```text
crates/media_io/src/
├── encode/        FFmpeg encoding raw frames from a pipe with the source's audio into Matroska
├── frame_queue/   a bounded queue between a decoder thread and its reader, and recycled buffers
├── lib.rs         the crate root: the programs to run, the error type and the module list
├── pcm_stream/    FFmpeg decoding audio to 32-bit float PCM, read in fixed-size chunks
├── preview/       FFmpeg command lines for a clip: its sound with a silence pad, and its frames
├── probe/         ffprobe's JSON for a video, and the choice of the English audio track
├── shot_changes/  FFmpeg's `scdet` scan: the times of the shot changes that cue timing snaps to
├── tests/         `lib.rs`'s own tests: `Programs::beside` picking the bundled pair or falling back
├── video_frames/  bounded frame streaming with presentation timestamps, stills and region crops
└── yuv/           YUV 4:2:0 frames in Rust: the source's colour, conversion to rgb24, luma reads
```

## How it works

The three modules serve the probe and decode stage: `probe` is for the streams, the frame rate
and the English track; `pcm_stream` for the audio at 16 kHz mono and 44.1 kHz stereo; and
`shot_changes` for a second FFmpeg process that scans the cuts. The visible-text stages read
`video_frames`: RGB frames paired with a presentation timeline taken from the packet table before
decoding, single stills at a timeline time, and runs of full-resolution frames cropped to one
region. The localize stage reads every frame at its native size in a raw pixel format from
`video_frames` and writes the new video through `encode`, which pipes raw frames into FFmpeg and
copies the source's audio, chapters and metadata beside them. `preview` builds the FFmpeg command
lines the app's line and text review run to play a clip's sound and picture. `Programs` names the `ffmpeg` and
`ffprobe` to run: the bare names on the `PATH` by default, or the pair bundled at
`<exe_dir>/ffmpeg/` when `Programs::beside`/`beside_current_exe` finds both there (`bundled` says
which). Every failure is a `MediaError`: the program could not run, exited non-zero, printed
something unreadable, or the video has no usable audio track.

## Public surface

- `probe::{probe, parse, english_track}`, `pcm_stream::{PcmStream, PcmRequest, PcmFormat,
  write_f32_file, read_f32_range, F32FileReader, F32FileWriter}`, `shot_changes::{scan, parse}`,
  `Programs` (with `beside` and `beside_current_exe`) and `MediaError`: for `crates/stages/`,
  `crates/pipeline/`, `apps/tbd_subtitles/`, `tools/stack_spike/`, `tools/stack_spike_ggml/` and
  `tools/visual_validation/`.
- `video_frames::{FrameStream, VideoFrame, Decode, PixelFormat, timeline}` (`FrameStream::open`,
  `open_native`, `timeline`, `next_frame`, `finish`), `video_frames::{YuvStream, YuvOptions}`
  (full-resolution yuv420p or NVDEC nv12 frames from any index, `spawn` onto a `FrameQueue`),
  `video_frames::pipe` (the enlarged frame pipe), `video_frames::still::still` and
  `video_frames::region::{RegionStream, RegionRequest, RegionCrop, RegionFrame}`: for the
  visible-text and localize stages in `crates/stages/` and the detection benchmark.
- `encode::{Encoder, EncodeSpec, VideoColour, EncoderProcess, available_encoder, encode_args,
  is_constant_frame_rate}`: for the localize stage in `crates/stages/src/localize/`.
- `yuv::{Yuv420, YuvFrame, Coefficients, Matrix, Range, to_rgb, to_rgb_parallel,
  to_rgb_padded_into, crop_to_rgb, luma_thumbnail, crop_grey}` and
  `frame_queue::{FrameQueue, Producer, BufferPool, PooledBuffer}`: for the visible-text and
  localize stages in `crates/stages/` and the detection benchmark.
- `preview::{Clip, track_sound, stem_sound, frame_size, frames}` and
  `preview::visual::{VisualPreview, args}`: for the app's line and text review players, and
  `frame_size` for on-screen text detection in `crates/stages/`.

## Boundaries

- Depends on: `child_process` for FFmpeg and ffprobe, `job_model` for the output types,
  `serde_json` for ffprobe's JSON, `rayon` for the colour conversion's row tasks, `libc` for the
  frame pipe's size.
- Used by: `crates/stages/`, `crates/pipeline/`, `apps/tbd_subtitles/`, `tools/stack_spike/`,
  `tools/stack_spike_ggml/` and `tools/visual_validation/`.
- Rules: no module holds a whole track in memory, and no module writes to the source video (the
  crate header in `lib.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#1-probe-and-decode) — what the probe stage
  reads and writes.
