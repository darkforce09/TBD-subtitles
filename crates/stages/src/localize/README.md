# Localized video

The stage of the `localized_video` step. It decodes every frame of the source at its native size,
blends the composed English patches of the baked occurrences over the frames they cover, and
streams the frames into an encoder that writes `<video>.localized.mkv` with the source's audio,
chapters and metadata.

## Contents

```text
crates/stages/src/localize/
├── blend.rs    patches as frame samples, and the alpha blend over 8-bit and 10-bit 4:2:0 frames
├── colour.rs   RGB to Y′CbCr in the stream's matrix, range and bit depth, and back
├── mod.rs      `render`: the decoder, the patch loop and the encoder; `frame_format`; the error
├── motion.rs   `Motion`: the writing's shift in each frame, folded from the `frames` rows
├── patches.rs  the frame-by-frame patch schedule, the byte-bounded patch cache and patch loading
├── still.rs    one region of one frame with its patches blended, back in RGB, for the read-back check
└── tests/      colour values, blend maths, shift runs, schedule and cache, stills, FFmpeg renders
```

## How it works

`render` refuses a stream without a frame rate, picks the encoder with
`media_io::encode::available_encoder` (NVENC HEVC when a one-frame test encode runs, else
libx264), and opens `FrameStream::open_native` at the probed size: 10-bit 4:2:0 frames
(`yuv420p10le`) for a 10-bit 4:2:0 source, 8-bit `yuv420p` for everything else, which FFmpeg
converts. A timeline that `is_constant_frame_rate` rejects is refused, since raw frames on a pipe
carry one constant rate. The encoder gets the stream's frame rate fraction, the first frame's start
as its offset and the stream's colour tags.

`Motion` folds the `frames` rows, read one at a time in key order, into runs of consecutive frames
of one plate that share one shift; it grows with the shift changes, never with the frames.
`Schedule` lists every plate with a patch of every baked occurrence, numbered in document order
and queued by first frame; a plate whose frames take shifts other than its own is listed as one
entry per run, each with the patch lettered at its shift (`Plate::shifted`), and a frame without a
row keeps the plate's own patch. Advancing to each frame ends the entries whose last frame has
passed and starts those whose first frame has come; the active ones stay in document order, so
overlapping patches stack as the document lists them. `PatchCache` loads a patch's RGBA PNG the
first time it is active, refuses one whose size differs from its plate's rectangle, converts it
once, and keeps it until the last entry that blends the file ends or, beyond 512 MiB of converted
samples, until it is the least recently used.

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

Progress is reported every 240 frames and at the end. The decoder and the encoder must both handle
exactly one frame per timeline entry; a cancel flag stops the loop and kills the encoder.

## Boundaries

- Depends on: `media_io::video_frames` (`FrameStream::open_native`, `PixelFormat`),
  `media_io::encode` (`EncodeSpec`, `EncoderProcess`, `available_encoder`, `VideoColour`,
  `is_constant_frame_rate`), `job_model::onscreen` (`ReplacementDocument`, `Plate`, `PixelRect`),
  `job_model::outputs::VideoStream` and `image` for the patch PNGs.
- Used by: `crates/pipeline/src/tasks/localized.rs`, which chooses the output path, guards an
  existing file, folds the `frames` rows into the `Motion` and renames the finished part file into
  place; `crates/pipeline/src/tasks/replace.rs` and `crates/pipeline/src/tasks/verify.rs` (the
  `Motion`, `frame_format` and the `Conversion`); composition
  (`onscreen_text::replace::compose`, one patch per shift of `Motion`); the read-back check
  (`onscreen_text::replace::verify`), which uses the schedule, `Motion` and `still`; and
  `tools/visual_validation/`.
- Rules: the source is only read; one frame and the active patches are held at a time, with the
  cache bounded by `CACHE_BYTES`; a variable frame rate is refused rather than drifting out of sync.

## Related documentation

- [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md) — how visible
  writing is detected, translated and presented.
- [Pipeline](/documentation/architecture/pipeline.md) — step order, workers and resume.
