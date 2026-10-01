**Status:** live

# Visual tracking and video acceleration

How replaced writing could follow moving signs more closely, erase them without flicker, and how
the localized video could re-encode only what changed. These are the items of milestone M8; each
builds on the per-frame `frames` table of `job.redb`
([binary storage](/documentation/architecture/binary_storage_plan.md)) and is built only when the
owner picks it.

## Where replacement stands

- **Following.** `text_mask` follows moving writing frame by frame: the keyframe window, at five
  scales from 0.9 to 1.1, is matched by normalised cross-correlation around the position the
  sampled quads suggest; one frame scoring under 0.8 loses the occurrence. Each frame's
  `FrameRecord` holds the keyframe quad carried to that frame, its follow score, a shift and a
  scale, its erase mask and its plate
  ([video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md)). Shift
  and scale cannot express rotation or perspective.
- **Erasing.** Frames whose placement and background stay alike share one
  [plate](/documentation/glossary.md#plate), inpainted once by LaMa; writing that travels a few
  pixels starts a new plate. Each plate is inpainted on its own.
- **The open failure.** On Dressrosa 11 the 海 wall in a panning newspaper photograph is no longer
  replaced: joining the shot's pieces into one occurrence makes its frames step between the
  pieces' boxes, and following fails ("The writing moves in a way that could not be followed")
  ([polish record](/documentation/research/localized_video_polish_dressrosa_28.md#open-issues)).
- **The encode.** The whole-video encode decodes every frame, blends the patches and re-encodes
  the video with `hevc_nvenc`: 265 s for Dressrosa 11's 44,489 frames, 166 frames per second
  ([measurement](/documentation/research/localized_video_dressrosa_11.md)). Few of those frames
  change: Dressrosa 11's `frames` table holds 370 rows, Dressrosa 28's 1,898
  ([per-frame tables](/documentation/research/per_frame_tables.md)). For a constant-frame-rate
  H.264 source, `localized_video` re-encodes only the segments that change (item 3), and keeps the
  whole-video encode as its fallback.

## 1. Per-frame homography

When the camera pans across writing or a sign tilts, a shift and a scale leave the lettering
sliding against the surface.

- **What changes:** following estimates a 3×3 perspective transform per frame, from the keyframe
  writing to that frame (a planar homography from tracked feature points, or optical flow inside
  the writing's window), and stores it in the frame's `FrameRecord`; the `frames` table's layout
  version rises with it. Composition already warps the lettering through the inverse homography
  of the keyframe quad; it uses each frame's own transform instead.
- **What it fixes:** writing that rotates or changes perspective, and the 海 wall, where one
  transform per frame follows the pan across the joined pieces instead of stepping between their
  boxes.
- **Measured:** the 海 wall followed and approved by `text_verify`; the follow scores and
  approved replacements of Dressrosa 11 and 28 against the baseline; `text_mask` time.

## 2. Keyframe inpaint and warped plates

A moving sign is split into several plates, each inpainted by LaMa on its own, so the filled
background can differ slightly from plate to plate.

- **What changes:** LaMa fills the keyframe's plate; for the other frames of the same shot the
  clean plate is warped along the per-frame homography of item 1. Where the warp no longer
  matches the frame (an occlusion, a cut, a follow score below threshold) a new plate is inpainted
  and blended across the change. Background the camera reveals in other frames of the shot can
  fill the mask from those frames before LaMa paints the rest.
- **What it fixes:** flicker between plates of one sign, and LaMa runs on every plate.
- **Measured:** LaMa calls and `text_inpaint` time against the baseline; the replacements approved
  by `text_verify`; the owner's review in Check Text.

## 3. Re-encoding only what changed

The Dressrosa sources are H.264 Main profile, level 4.0 (ffprobe on 11, 28 and 40). Dressrosa 11
is `yuv420p`, 1920×1080 at 24 fps with B-frames, with a keyframe every 3.4 s on average and at most
10 s apart.

Built, awaiting the host measurement
([decision](/documentation/decisions/localized_video.md#2026-10-01--the-localized-video-re-encodes-only-the-segments-with-replaced-writing-as-h264-matching-the-source-and-copies-the-rest);
how it runs: [video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md#the-localized-video-localized_video)).

- **Which sources:** a constant-frame-rate H.264 source takes the segment encode; any other
  constant-frame-rate source goes to the whole-video HEVC encode, and the report line says why; a
  variable frame rate is refused as before.
- **Segments:** the frame spans with patches come from the blend schedule; each span widens to the
  IDR keyframes around it, read from the packets' keyframe flags and confirmed by a Rust scan of
  the boundary packet's NAL units (an open group of pictures widens to the next IDR), and
  overlapping spans merge. Only those segments are decoded from their keyframe, blended and
  re-encoded; the ranges between them are stream-copied.
- **The constraint:** stream-copied H.264 and re-encoded segments join into one playable stream
  only if the re-encoded segments are H.264 too, matching the source's profile, level, pixel
  format, size, sample aspect ratio, colour tags, B-frames and reference frames (read with
  ffprobe), opening on an IDR with closed groups of pictures, and each piece carries its stream
  headers in-band before every keyframe, since the concat demuxer keeps only the first file's
  out-of-band headers. The encoder is libx264 with `stitchable=1:repeat-headers=1` by default, or
  `h264_nvenc` with `-repeat_headers 1` when the setting asks for it (a 10-bit H.264 source always
  takes x264), its peak rate capped at 1.5 times the source's and at the level's maximum; copied
  pieces get their headers from `dump_extra=freq=keyframe`.
- **The join:** FFmpeg's concat demuxer over the pieces with the source's audio, chapters and
  metadata copied, and the source's video start offset carried over with `-itsoffset`.
- **Checks, failing closed:** the frame count and duration equal the source's, the video–audio
  offset matches the source's within half a frame, a decode around each join reports no error,
  and the copied pieces' packets equal the source's apart from the in-band headers. Any failure
  falls back to the whole-video encode, with its reason in the record and the report.
- **What it keeps:** frames without replaced writing keep the source's bitstream exactly.
- **What the host measures:** the encode-bench sets the x264 and NVENC presets (`-preset slow` and
  `p7 -tune hq` until then; the whole-video HEVC keeps `p6`); `localized_video` time and size
  against the baseline on Dressrosa 11 and 28; playback in VLC and mpv across every join (no
  stall, no corrupt frame, audio in sync); the frame count equal to the source's
  ([runbook](/documentation/runbooks/measuring_full_resolution_screening.md)).

## 4. Smoothing and fragments

- **Smoothing:** the per-frame shifts (or homographies) of moving writing are smoothed over time,
  with a Kalman filter or a similar smoother over the `frames` rows, so the lettering does not
  jitter where the correlation peak wobbles; a frame the smoother moves too far from its match
  stays as matched.
- **Fragments:** the scan keeps short fragments apart from their occurrence, such as the two-frame
  start of the Rebecca cards at their fade-in on Dressrosa 11
  ([visual scan](/documentation/research/visual_scan_dressrosa_11.md)); a fragment adjacent to an
  occurrence of the same writing joins it.
- **Cuts already bound occurrences:** the scan screens both frames around every shot cut and
  bisects to the exact entry and exit frame, so no occurrence crosses a cut.
- **Measured:** jitter of the lettering on the moving signs of Dressrosa 11 and 28 (the frame to
  frame movement of its quad against the follow), the fragments left, and the owner's review.

## Boundaries

- Depends on: [video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md),
  [media_io](/crates/media_io/README.md), and the `frames` table of
  [binary storage](/documentation/architecture/binary_storage_plan.md).
- Used by: roadmap milestone M8, `stages::localize` and `pipeline::tasks::localized`.
- Rules: source videos stay read-only; replaced writing appears on the exact frame the picture
  changes, which detection finds by bisection; under the segment encode, frames without replaced
  writing keep their bitstream; a segment join that fails a check falls back to the whole-video
  encode, never to an unchecked file.

## Related documentation

- [Roadmap](/documentation/roadmap.md#m8--visual-tracking-and-video-acceleration) — milestone M8.
- [Memory profiles](memory_profiles.md) — the baseline, hardware decoding and overlapped encoding.
- [Measuring full-resolution screening](/documentation/runbooks/measuring_full_resolution_screening.md)
  — the host steps, the encode-bench and the playback checks across the joins among them.
- [Localized video polish on Dressrosa 28](/documentation/research/localized_video_polish_dressrosa_28.md)
  — the open issues these items address.
