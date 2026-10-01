# Video encode

FFmpeg encoding raw frames from a pipe into a Matroska file beside the source video: the new video
stream, the source's audio copied unchanged, its chapters and metadata, and no subtitle stream.
For an H.264 source, `segments` re-encodes only the pieces with changed frames and joins them to
the source's copied packets instead.

## Contents

```text
crates/media_io/src/encode/
├── mod.rs      the encoder choice, the encode spec, its command line, colour tags, the rate check
├── process.rs  `EncoderProcess`: FFmpeg reading whole frames on stdin, reaped with its reason
├── segments/   the segment encode: eligibility, IDR keyframes, pieces, copy, join and checks
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
frames are encoded as `yuv420p`. When the source's bit rate is known, `-maxrate` caps the peak
rate at 1.25 times it for NVENC and 1.5 times it for libx264, with a buffer of twice the peak, so
the localized video stays near the source's size. Each colour tag the probe knows (`VideoColour::of` a probed
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
last 4 KiB of FFmpeg's stderr. `EncoderProcess::start_segment` drives one re-encoded piece the same
way, with `segments::segment_args` in place of `encode_args`: raw frames in, a Matroska file of
video alone out. Either is started, written and finished from one thread.

`segments` builds the localized video from pieces when the source is constant-rate H.264: the
frames with replaced writing are widened to the IDR keyframes around them and re-encoded as H.264
matching the source, with their headers in-band; every other frame's packets are copied; the
concat demuxer joins the pieces with the source's audio; and the joined file is checked before it
is installed. Any doubt is a `FallbackReason`, and the caller encodes the whole video as above.

## Public surface

- `Encoder`, `EncodeSpec`, `VideoColour`, `available_encoder`, `encode_args`,
  `is_constant_frame_rate`, `EncoderProcess` (`start`, `start_segment`, `write_frame`,
  `frame_bytes`, `frames_written`, `finish`): the whole-video encode and the frame pipe both
  encodes share.
- `segments`, for the localize stage's segment encode:
  - probing and eligibility: `probe_h264_source(&Programs, &Path) -> Result<H264Source,
    MediaError>` (with `H264Source::{fps, frame_s}` and `starts: StreamStarts`),
    `stream_starts`, `segment_eligibility(&H264Source, timeline, keyframes) -> Result<(),
    FallbackReason>`;
  - keyframes and IDRs: `keyframes` and `keyframe_packets(&Programs, &Path) ->
    Result<Vec<KeyframePacket>, MediaError>` (from `video_frames::packets`),
    `IdrProbe::new(&Programs, &Path, PacketFormat, Vec<KeyframePacket>, frame_s)`,
    `IdrProbe::confirm(&mut self, &[u64]) -> Result<Vec<bool>, MediaError>`,
    `IdrProbe::confirmed`; `nal_unit_types`, `starts_idr_picture`, `parse_hex_dump`;
  - planning: `plan_pieces(&[FrameSpan], keyframes, frame_count, IdrCheck) -> Result<Vec<Piece>,
    PlanError>`, `Piece::{Copy, Encode}` with `first`, `last`, `frames`, `is_copy`, and
    `piece_summary(&[Piece]) -> SegmentSummary`;
  - pieces and join: `copy_pieces(&Programs, source, &[Piece], folder, timeout, cancel)`,
    `piece_file(folder, number, Piece) -> PathBuf`, `SegmentSpec::for_source(&H264Source,
    PixelFormat, LocalizedEncoder, output) -> Result<SegmentSpec, FallbackReason>`,
    `segment_encoder(&Programs, LocalizedEncoder, &H264Source) -> LocalizedEncoder`,
    `join_pieces(&Programs, &[Piece], &JoinRequest, timeout, cancel)`;
  - checks: `verify_join(&Programs, &VerifyRequest) -> Result<(), FallbackReason>`;
  - the presets and qualities `X264_SEGMENT_PRESET`, `X264_SEGMENT_CRF`, `NVENC_SEGMENT_PRESET`,
    `NVENC_SEGMENT_CQ`, which a bench may override in `SegmentSpec::preset`.

The stage calls them in this order, falling back to the whole-video encode on any reason or
error:

```text
source   = probe_h264_source(programs, video)?
timeline = video_frames::timeline(programs, video, stream.start_time_s, source.fps())?
keys     = keyframe_packets(programs, video)?            indices = keys[..].index
segment_eligibility(&source, &timeline, &indices)        Err(reason) -> whole encode
probe    = IdrProbe::new(programs, video, source.packet_format?, keys, source.frame_s())
pieces   = plan_pieces(&changed_spans, &indices, timeline.len(), &mut |b| probe.confirm(b))?
copy_pieces(programs, video, &pieces, folder, deadline, cancel)?
encoder  = segment_encoder(programs, settings.localized_encoder, &source)
for (n, Encode { first, last }) in pieces:
    spec = SegmentSpec::for_source(&source, frame_format, encoder, piece_file(folder, n, piece))?
    process = EncoderProcess::start_segment(programs, &spec, deadline, cancel)?
    write frames first..=last (decoded from the keyframe at `first`, patches blended)
    process.finish()? == piece.frames()
join_pieces(programs, &pieces, &JoinRequest { folder, source: video, video_offset_s:
            timeline[0].0, frame_rate: source.frame_rate, output: <video>.localized.mkv.part },
            deadline, cancel)?
verify_join(programs, &VerifyRequest { source: video, source_timeline: &timeline,
            source_starts: source.starts, frame_rate: source.frame_rate, pieces: &pieces,
            output: part, folder, timeout })                Err(reason) -> whole encode
install part; record piece_summary(&pieces)
```

## Boundaries

- Depends on: `child_process::Run` (with `stdin_piped`) for FFmpeg and ffprobe,
  `video_frames::{PixelFormat, packets, timeline}` for the raw layout, the keyframes and the frame
  times, `job_model::outputs::VideoStream` for the colour tags, and
  `job_model::onscreen::{LocalizedEncoder, SegmentSummary}` for the segment encoder and its
  summary.
- Used by: the localize stage in `crates/stages/src/localize/`, which the localized-video task in
  `crates/pipeline/src/tasks/localized.rs` runs.
- Rules: the source is only read; the output never holds a subtitle stream
  (`a_round_trip_keeps_every_frame_and_the_audio_and_drops_the_subtitles` in `tests/process.rs`);
  a known source rate caps the peak rate (`a_known_source_rate_caps_the_peak_rate_per_encoder` in
  `tests/encode.rs`); only whole frames are
  written (`a_frame_of_the_wrong_size_is_refused`); a cancel kills FFmpeg even mid-write
  (`a_cancel_stops_the_encode_and_says_so`); a joined video is installed only after every
  segment check passes (the rules in `segments/README.md`).

## Related documentation

- [Video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md) — the
  localized video this encode writes.
- [Visual and video optimizations](/documentation/optimizations/visual_and_video.md#3-re-encoding-only-what-changed)
  — re-encoding only the segments that change.
- [Development environment](/documentation/runbooks/development_environment.md#host-and-container)
  — the container's FFmpeg has no CUDA, so it encodes with libx264; the host uses NVENC.
