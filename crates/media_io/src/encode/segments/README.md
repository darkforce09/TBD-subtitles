# Segment encode

The media side of a localized video that re-encodes only the frames with replaced writing: the
source's H.264 stream probed and judged, its IDR keyframes found, the frames laid out as copied
and re-encoded pieces, the copied pieces cut with their headers in-band, the segment encode that
matches the source, the join with the source's audio, and the checks before install.

## Contents

```text
crates/media_io/src/encode/segments/
├── args.rs     `SegmentSpec`, the copy and segment command lines, the segment encoder choice
├── copied.rs   the copied packets of the joined file compared with the source's, by `framemd5`
├── idr.rs      `IdrProbe`: which keyframes open an IDR picture, read from their packets' bytes
├── join.rs     piece files, the copy run, the concat list with exact durations, the join
├── mod.rs      the module tree, the re-exports, `FallbackReason` and the ffprobe runner
├── nal.rs      NAL unit types of an Annex B or length-prefixed packet, ffprobe's hex dumps
├── plan.rs     `plan_pieces`: changed frames widened to IDR keyframes and laid out as pieces
├── profile.rs  H.264 profiles and levels as both encoders name them, the level's peak rate
├── source.rs   `H264Source`, when video and audio start, and the eligibility check
├── verify.rs   `verify_join`: frame times, video-to-audio offset, decode around joins
└── tests/      plans, NAL scans, arguments, probes, FFmpeg round trips with real joins (NVENC on host)
```

## How it works

```text
probe_h264_source ─▶ segment_eligibility ──(reason)──▶ whole-video encode
        │
keyframe_packets ─▶ IdrProbe::confirm ◀── plan_pieces(changed spans) ─▶ [Copy | Encode]…
                                                     │
          copy_pieces (one FFmpeg run, -f segment) ◀─┤
          EncoderProcess::start_segment per Encode ◀─┘   (the stage renders the frames)
                                                     │
                     join_pieces (concat list, -itsoffset) ─▶ <output>.part
                                                     │
                     verify_join ──(reason)──▶ whole-video encode, else install
```

`source.rs` reads the first video stream with one ffprobe: codec, profile, level, reference
frames, reorder depth (`has_b_frames`), pixel format, size, sample aspect ratio, colour tags,
nominal and average rates, field order, packet format (`is_avc` and `nal_length_size`) and the
bit rate (the stream's, mkvmerge's `BPS` tag, or the file's). Two short packet reads give the first
presented video frame and the first audio packet, discarded or not, on the file's own clock.
`segment_eligibility` accepts only progressive 4:2:0 H.264 in Baseline, Main, High or High 10 at a
known level and packet format, with an even size, a constant rate (nominal and average agree and
`is_constant_frame_rate` holds over the timeline) and a keyframe on frame 0.

`video_frames::packets::keyframe_packets` lists the keyframe flags of the packet table, numbered
as the timeline numbers frames. A flag is not enough: an open-GOP recovery point is flagged too.
`IdrProbe` seeks ffprobe to a quarter frame after each asked keyframe, reads one packet with
`-show_data`, checks that it presents within half a frame of the keyframe, and `nal.rs` scans its
units: the keyframe is an IDR only when its first slice is an IDR slice (`nal_unit_type` 5).

`plan_pieces` merges overlapping and touching changed spans, then walks each span's start back to
the last IDR keyframe at or before it (frame 0 needs no check) and its end forward to the first
IDR keyframe after it, asking the probe in ascending batches and never twice. Pieces that meet
become one; the gaps are copied. At an IDR keyframe the decode order and the presentation order
cut the stream at the same frame, which is what makes each cut exact.

`copy_pieces` runs one stream copy of the source through `h264_mp4toannexb,dump_extra=freq=keyframe`
into the segment muxer, cut with `-segment_frames` at every piece's first frame; the cuts that
re-encoded pieces replace are removed. Each re-encoded piece is a separate FFmpeg reading raw
frames (`segment_args`): x264 `stitchable=1:repeat-headers=1`, closed GOPs, no B-pyramid, the
source's profile, level and reference count, `-forced-idr 1`, the sample aspect ratio and colour
tags, a peak rate of `H264_PEAK_SHARE` times the source's capped by the level (`peak_rate`), preset
`X264_SEGMENT_PRESET`; or NVENC `p7 -tune hq` with `dump_extra` putting its headers in-band.

`join_pieces` writes `pieces.ffconcat`, each piece with the duration its frames take at the
constant rate (rounded to the microsecond at each piece's start, so nothing accumulates), and runs
the concat demuxer with `-auto_convert 0`, since the pieces carry their own headers, offset by the
source video's start (`-itsoffset`, `timeline[0].0`), with every audio stream, the chapters and the
metadata of the source copied into Matroska.

`verify_join` fails closed, cheapest check first: the joined timeline has the source's frame count
and every frame's time within half a frame; the video starts as far from the audio as in the
source, within half a frame; a `-v error` decode from an IDR at least `DECODE_LEAD_S` before each
join to `DECODE_LAG_S` after it prints nothing; and every copied packet's bytes are the source's,
or longer with only SPS and PPS added (`framemd5` as stored and through
`filter_units=remove_types=7|8`).

## Boundaries

- Depends on: the parent `encode` module (`VideoColour`, `is_constant_frame_rate`, the colour
  arguments, `H264_PEAK_SHARE`, `EncoderProcess`), `video_frames::{packets, timeline}`,
  `child_process::Run` for ffprobe and FFmpeg, `job_model::onscreen::{LocalizedEncoder,
  SegmentSummary}`, and `serde_json` for ffprobe's JSON.
- Used by: `EncoderProcess::start_segment` in `crates/media_io/src/encode/process.rs`; the
  localize stage in `crates/stages/src/localize/`.
- Rules: every piece starts at frame 0 or a confirmed IDR keyframe and the pieces cover every frame
  once (`every_plan_covers_each_frame_once_and_starts_every_piece_where_a_decoder_can` in
  `tests/plan.rs`); a keyframe counts as an IDR only from its own packet bytes
  (`open_gop_keyframes_are_not_idr` in `tests/idr.rs`); a segment whose headers differ from the
  source's joins cleanly and leaves the copied frames unchanged
  (`a_segment_with_different_headers_joins_cleanly_and_copies_the_rest_unchanged` in
  `tests/segments.rs`); pieces without in-band headers and a mis-set offset fail the checks
  (`pieces_cut_without_in_band_headers_fail_the_decode_check`,
  `an_offset_source_keeps_its_video_start_and_a_wrong_offset_fails_the_sync_check`).

## Related documentation

- [Visual and video optimizations](/documentation/optimizations/visual_and_video.md#3-re-encoding-only-what-changed)
  — why only the changed segments are re-encoded, and the constraints of joining H.264.
- [Video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md) — the
  localized video these pieces make.
