**Status:** live

# Decisions: the localized video

The decisions about how the localized video, `<video>.localized.mkv`, is written once its
patches are composed: which frames are re-encoded, by which encoder, and what is checked before
the file is kept. Replacing the writing in a localized video at all is the
[2026-09-30 entry](/documentation/decisions/stack_and_pipeline.md#2026-09-30--writing-is-replaced-in-a-localized-video-re-encoded-beside-the-source)
of the stack and pipeline decisions. The [decision log](/documentation/decisions/) says how
entries are written; the pipeline is the
[video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md).

### 2026-10-01 — The localized video re-encodes only the segments with replaced writing, as H.264 matching the source, and copies the rest

**Context:** On Dressrosa 11 the whole-video HEVC encode took 265 s at 166 fps with the single
NVENC engine busy 99–100 %, and 89 % of the step waited on the encoder pipe
([baseline](/documentation/research/m6_baseline.md#phases)); decode and blend took the rest on
the same thread. Most frames carry no replaced writing, yet every frame was decoded, piped and
encoded again, a second generation of loss for frames that do not change. The source's own H.264
stream holds those frames exactly. The options were: keep encoding every frame and only hide the
decode behind queues; or re-encode the frames with replaced writing and copy every other frame's
packets, which needs re-encoded pieces that a decoder can cross into and out of. H.264 joins
cleanly only at IDR pictures, the concat demuxer keeps only the first file's out-of-band headers,
and an MP4 edit list or a B-frame delay can start the video later than the audio.

**Decision:** For a constant-frame-rate H.264 source (Baseline, Main, High or High 10, 4:2:0,
progressive, even size, a known level and packet format, a keyframe on frame 0), the localized
video is built from pieces:

- **Spans:** every frame any patch is active on, from the patch schedule, widened to the IDR
  keyframes around it; overlapping and touching spans merge. A keyframe counts as an IDR only when
  its own packet's first slice is an IDR slice (`nal_unit_type` 5); an open-GOP keyframe widens the
  span to the next IDR.
- **Copied pieces:** the source's packets, stream-copied through
  `h264_mp4toannexb,dump_extra=freq=keyframe` and cut with the segment muxer, so every piece
  carries its SPS and PPS in-band before each keyframe.
- **Re-encoded segments:** decoded from their first frame, blended, and encoded by x264 by default
  with `stitchable=1:repeat-headers=1`, closed GOPs, no B-pyramid, an IDR first frame, and the
  source's profile, level, reference frames, sample aspect ratio and colour tags, preset `slow`;
  the peak rate is 1.5× the source's, capped at the level's maximum. The setting can choose NVENC
  (`h264_nvenc` `p7 -tune hq`), whose headers `dump_extra` puts in-band; a 10-bit source always
  uses x264.
- **Join:** each piece is its own Matroska file; the concat demuxer joins them with
  `-auto_convert 0`, each piece given the exact duration of its frames, with every audio stream,
  the chapters and the metadata of the source copied; the source's video start is restored with
  `-itsoffset`.
- **Checks before the file is kept:** the joined frame count and each frame's time match the
  source's; the video starts as far from the audio as in the source, within half a frame; a
  `-v error` decode of a few seconds around each join prints nothing; the copied packets are
  byte-identical to the source's apart from the in-band headers.

Any failure — an ineligible source, a plan, copy, encode or join that fails, a check that fails,
or a plan whose every piece is re-encoded — falls back to the whole-video encode, which stays
HEVC (`hevc_nvenc` p6, else libx264), and the record names the reason. A variable frame rate is
still refused. The decode, blend and encode run on three threads joined by bounded queues.

**Consequences:** For an eligible source the localized video is H.264, not HEVC, and every frame
without replaced writing keeps the source's bitstream exactly, so only the changed segments lose
a generation. The record and the report give the segments re-encoded, the frames re-encoded and
copied, and the fallback reason. A job writes its copied pieces under the job folder while the
step runs, about the size of the source's video stream, and removes them after. The time saved
is to be measured on the host, the presets set by `encode-bench`, and the joins checked in VLC
and mpv. `only_the_segment_with_a_patch_is_re_encoded_and_the_rest_decodes_unchanged` (stages),
`a_segment_with_different_headers_joins_cleanly_and_copies_the_rest_unchanged` and
`an_offset_source_keeps_its_video_start_and_a_wrong_offset_fails_the_sync_check` (media_io) hold
it in place.

**Supersedes:** the whole-video HEVC encode part of
[2026-09-30 — Writing is replaced in a localized video, re-encoded beside the source](/documentation/decisions/stack_and_pipeline.md#2026-09-30--writing-is-replaced-in-a-localized-video-re-encoded-beside-the-source),
which now applies only when the segments cannot be used.
