# Localized video

The stage of the `localized_video` step. It blends the composed English patches of the baked
occurrences over the frames they cover and writes `<video>.localized.mkv` with the source's
audio, chapters and metadata: for a constant-rate H.264 source, only the segments with replaced
writing are re-encoded, as H.264 matching the source, and every other frame is copied; otherwise,
or on any doubt, every frame is decoded, blended and re-encoded.

## Contents

```text
crates/stages/src/localize/
├── blend.rs     patches as frame samples, and the alpha blend over 8-bit and 10-bit 4:2:0 frames
├── colour.rs    RGB to Y′CbCr in the stream's matrix, range and bit depth, and back
├── frames.rs    the decode thread: `YuvStream` for 8-bit frames, filtered native frames for 10-bit
├── mod.rs       `render`, the request, the result and phase times, the `Blender`, `frame_format`
├── motion.rs    `Motion`: the writing's shift in each frame, folded from the `frames` rows
├── patches.rs   the frame-by-frame patch schedule, its changed spans, the patch cache and loading
├── segments.rs  the segment encode: plan, copy, render each re-encoded piece, join, check
├── still.rs     one region of one frame with its patches blended, back in RGB, for the read-back check
├── threads.rs   decode → blend → encode: the step thread, the encoder thread and their queue
├── whole.rs     the whole-video encode with the best available encoder
└── tests/       colour, blend, shifts, schedule and spans, stills, fake threads, fallbacks, FFmpeg renders
```

## How it works

`render` refuses a stream without a frame rate, reads the source's timeline once and refuses a
timeline that `is_constant_frame_rate` rejects, since raw frames on a pipe carry one constant
rate. Frames are blended in `frame_format`: 10-bit 4:2:0 (`yuv420p10le`) for a 10-bit 4:2:0
source, 8-bit `yuv420p` for everything else, which FFmpeg converts. It then tries the segment
encode and settles (`settle`) on its video, on its stop (a cancel), or on the whole-video encode
with the reason the segments were not used.

```text
segments::encode ─ plan (probe_h264_source, keyframe_packets, segment_eligibility, IdrProbe,
   │                     plan_pieces over the schedule's changed spans, a copied piece needed)
   ├─ copy_pieces ─ per Encode piece: Decoder → Blender → encoder thread (start_segment)
   ├─ join_pieces ─ verify_join ─▶ Rendered { segments: piece_summary }
   └─ any FallbackReason ─▶ whole::encode (available_encoder) ─▶ segments: reason recorded
```

`segments.rs` follows `media_io::encode::segments`: the source must be constant-rate H.264 in a
profile and level a segment can match; the changed spans are every frame any scheduled patch
covers (`Schedule::changed_spans`), widened by `plan_pieces` to the IDR keyframes around them; a
plan with no copied piece gains nothing and falls back. The pieces live in the request's
`pieces_dir`, made afresh and removed when the attempt ends. The copied pieces are cut in one
FFmpeg run; each re-encoded piece is rendered through the threads below into
`EncoderProcess::start_segment` with the encoder `segment_encoder` picks from the setting (x264,
or NVENC when asked and it runs; x264 for a 10-bit source); the pieces are joined with the
source's audio into the output and `verify_join` checks it. A failure of any of these, or a
reason from any check, removes the output and falls back; a cancel stops the render. With no
patch at all, every frame is copied and the encoder is `copy`. `whole.rs` encodes every frame with
`media_io::encode::available_encoder` (NVENC HEVC when a one-frame test encode runs, else
libx264), with the stream's frame rate fraction, the first frame's start as its offset and the
stream's colour tags, and records one re-encoded segment of every frame with the reason.

`threads::run_frames` moves one run of frames — the whole video, or one re-encoded piece — through
three threads. The decode thread (`frames::Decoder`) runs on a `FrameQueue` about four seconds of
frames ahead: `YuvStream` from the run's first frame for 8-bit frames, into pooled buffers; for
10-bit frames, `FrameStream::open_native` from the start, handing out only the asked ranges (one
decoder serves every piece of a segment encode). The step thread takes each frame, checks it is
the next index, blends it in place (`Blender::apply`) and hands it over a channel of
`ENCODE_QUEUE_FRAMES` frames to the encoder thread, which starts the encoder, writes every frame
and finishes it, all on its own thread, as `child_process` requires. An error on any thread stops
the run and comes back from it, the encoder's exit explaining a write it refused; a stopped run
kills the encoder rather than finishing a partial file.

`Motion` folds the `frames` rows, read one at a time in key order, into runs of consecutive frames
of one plate that share one shift; it grows with the shift changes, never with the frames.
`Schedule` lists every plate with a patch of every baked occurrence, numbered in document order
and queued by first frame; a plate whose frames take shifts other than its own is listed as one
entry per run, each with the patch lettered at its shift (`Plate::shifted`), and a frame without a
row keeps the plate's own patch. Advancing to each frame ends the entries whose last frame has
passed and starts those whose first frame has come; the active ones stay in document order, so
overlapping patches stack as the document lists them. Frames skipped between re-encoded pieces
carry no patch. `PatchCache` loads a patch's RGBA PNG the first time it is active, refuses one
whose size differs from its plate's rectangle, converts it once, and keeps it until the last
entry that blends the file ends or, beyond 512 MiB of converted samples, until it is the least
recently used.

`colour` converts R′G′B′ with the matrix the stream is tagged with (`bt709`; `bt470bg` and
`smpte170m` as BT.601; `bt2020nc`), else BT.709 from 720 lines and BT.601 below, in limited range
unless tagged `pc`, scaled to 8 or 10 bits. `blend` mixes luma per pixel with the pixel's alpha and
chroma per 2×2 block with the block's summed alpha, counting pixels outside the patch as clear, so
a patch at any position and of any size covers each chroma sample by its share. Integer rounding
keeps a clear pixel's bytes identical and writes an opaque pixel's value exactly.

`still::finished_region` shows one region of one frame as the video will: the region, on even
pixels so its chroma blocks are the frame's, goes to 8-bit 4:2:0 samples in the stream's
conversion, each patch part inside it is converted and blended by `blend` exactly as the render
does, and `Conversion::rgb` turns the samples back into R′G′B′. The read-back check reads it.

Progress is reported every 240 frames and at the end, over the frames the render re-encodes. The
render times its phases into `Rendered::phases` (`RenderPhases`), summed over every run: the step
thread waiting on the decode thread, advancing the schedule with loading and blending the patches,
and waiting for room in the encoder thread's queue; the flush after each run's last frame that
drains the encoder thread and ends the decoder; and the segment encode's plan, copy, join and
check. The task notes them as `decode_wait_s`, `blend_s`, `encode_wait_s`, `flush_s`, `plan_s`,
`copy_s`, `join_s` and `verify_s`.

## Boundaries

- Depends on: `media_io::video_frames` (`YuvStream`, `FrameStream::open_native`, `timeline`,
  `PixelFormat`), `media_io::frame_queue` (`FrameQueue`, `PooledBuffer`), `media_io::encode`
  (`EncodeSpec`, `EncoderProcess`, `available_encoder`, `VideoColour`, `is_constant_frame_rate`,
  and `segments` for the whole segment encode), `job_model::onscreen` (`ReplacementDocument`,
  `Plate`, `PixelRect`, `LocalizedEncoder`, `SegmentSummary`), `job_model::outputs::VideoStream`
  and `image` for the patch PNGs.
- Used by: `crates/pipeline/src/tasks/localized.rs`, which chooses the output path and the pieces'
  folder, passes the encoder setting, guards an existing file, folds the `frames` rows into the
  `Motion`, renames the finished part file into place and records the segment summary;
  `crates/pipeline/src/tasks/replace.rs` and `crates/pipeline/src/tasks/verify.rs` (the
  `Motion`, `frame_format` and the `Conversion`); composition
  (`onscreen_text::replace::compose`, one patch per shift of `Motion`); the read-back check
  (`onscreen_text::replace::verify`), which uses the schedule, `Motion` and `still`; and
  `tools/visual_validation/` (`frame_format` for `encode-bench`).
- Rules: the source is only read; the decode queue, the encoder queue and the active patches
  bound what is held, with the cache bounded by `CACHE_BYTES`; a variable frame rate is refused
  rather than drifting out of sync; the encoder lives on one thread from start to finish
  (`frames_reach_the_encoder_blended_in_order_on_one_thread_of_its_own` in `tests/threads.rs`);
  only the segment with a patch is re-encoded and the copied frames decode unchanged
  (`only_the_segment_with_a_patch_is_re_encoded_and_the_rest_decodes_unchanged` in
  `tests/segments.rs`); every fallback reason runs the whole-video encode and records the reason
  (`every_fallback_reason_runs_the_whole_encode_with_that_reason_and_the_time_spent`).

## Related documentation

- [Localized video decisions](/documentation/decisions/localized_video.md) — why only the changed
  segments are re-encoded, and the checks before the joined video is kept.
- [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md) — how visible
  writing is detected, translated and presented.
- [Pipeline](/documentation/architecture/pipeline.md) — step order, workers and resume.
