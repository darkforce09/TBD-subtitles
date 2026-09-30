# Video encode

FFmpeg encoding raw frames from a pipe into a Matroska file beside the source video: the new video
stream, the source's audio copied unchanged, its chapters and metadata, and no subtitle stream.

## Contents

```text
crates/media_io/src/encode/
├── mod.rs      the encoder choice, the encode spec, its command line, colour tags, the rate check
├── process.rs  `EncoderProcess`: FFmpeg reading whole frames on stdin, reaped with its reason
└── tests/      exact command lines, the rate check, and FFmpeg round trips on a generated source
```

## How it works

`available_encoder` lists FFmpeg's encoders. When `hevc_nvenc` is listed and a one-frame test
encode of a 256x256 colour source succeeds, NVENC HEVC is the encoder; otherwise libx264 is, and
an FFmpeg with neither is an error. The test encode matters because FFmpeg lists NVENC whenever it
was built with it, even without an NVIDIA driver. NVENC refuses very small frames (a 64x36 frame
fails), which a real video never is.

`encode_args` turns an `EncodeSpec` into one command line. Input 0 is raw video on `pipe:0` in the
spec's pixel format, size and constant rate (`-framerate N/D`), shifted by `-itsoffset` to the time
the source's first frame presents. Input 1 is the source: `-map 1:a?` copies every audio stream,
`-map_chapters 1` and `-map_metadata 1` keep its chapters and tags, and `-sn -dn` leave out
subtitle and data streams. NVENC runs `-preset p6 -tune hq -rc vbr -cq 19 -b:v 0` with the `main`
or `main10` profile; libx264 runs `-preset slow -crf 16`, with `high10` for 10-bit frames; rgb24
frames are encoded as `yuv420p`. Each colour tag the probe knows (`VideoColour::of` a probed
`VideoStream`) is written; an unknown one is left out. `-y` overwrites a stale output from an
interrupted run, and `-max_muxing_queue_size 4096` keeps the copied audio from overflowing while
the video encoder starts.

Raw frames carry one constant rate, so `is_constant_frame_rate` checks the source timeline first:
each frame must start where `index / fps` puts it, within 1 % of a frame plus a millisecond, the
rounding of Matroska's millisecond clock. A caller that finds a variable rate does not encode.

`EncoderProcess::start` validates the spec (a 4:2:0 frame needs an even size, the rate a non-zero
fraction, the output must differ from the source), starts FFmpeg with a piped stdin, a deadline
and an optional cancel flag. `write_frame` takes exactly one frame of the spec's size; a failed
write means FFmpeg stopped reading. `finish` closes stdin so FFmpeg flushes the file, reaps it,
and returns the frames written, the deadline or cancel as a run error, or the exit code with the
last 4 KiB of FFmpeg's stderr.

## Boundaries

- Depends on: `child_process::Run` (with `stdin_piped`) for FFmpeg, `video_frames::PixelFormat`
  for the raw layout, `job_model::outputs::VideoStream` for the colour tags.
- Used by: the localized-video task in `crates/pipeline/`.
- Rules: the source is only read; the output never holds a subtitle stream
  (`a_round_trip_keeps_every_frame_and_the_audio_and_drops_the_subtitles`); only whole frames are
  written (`a_frame_of_the_wrong_size_is_refused`); a cancel kills FFmpeg even mid-write
  (`a_cancel_stops_the_encode_and_says_so`).

## Related documentation

- [Video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md) — the
  localized video this encode writes.
- [Development environment](/documentation/runbooks/development_environment.md#host-and-container)
  — the container's FFmpeg has no CUDA, so it encodes with libx264; the host uses NVENC.
