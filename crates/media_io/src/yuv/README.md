# YUV frames

Decoded YUV 4:2:0 frames handled in Rust: the source's own matrix and range, conversion to rgb24
whole, padded below to a batch's height or cropped to one rectangle, and the brightness plane read
alone for duplicate checks and grey crops.

## Contents

```text
crates/media_io/src/yuv/
├── colour.rs   `Matrix`, `Range` and `Coefficients`: the stream's colour as 13-bit integer coefficients
├── convert.rs  `Yuv420` pictures (planar or nv12) and their conversion: whole, padded or one rectangle
├── frame.rs    `YuvFrame`: one decoded frame with its index, times, size, layout and pooled samples
├── luma.rs     `LumaThumbnail` of cell means and `crop_grey`, read from the brightness plane alone
├── mod.rs      the module root and its public names
└── tests/      conversion per matrix and range, serial against parallel, padding, crops and thumbnails
```

## How it works

FFmpeg hands frames over as 8-bit yuv420p, half the bytes of rgb24, or as nv12 from NVDEC, and
a 10-bit source is decoded to 8 bits first. `Coefficients::of` reads the stream's `color_space`
and `color_range` tags (BT.709 from 720 lines and limited range when untagged) and turns them into
13-bit fixed-point factors, so every pixel is converted with integer arithmetic and the same bytes
come out on one thread or one row per rayon task. `to_rgb_padded_into` writes a frame into a
reused batch buffer whose height is a multiple of 32 and paints the rows below it black, which is
how the detectors take full-resolution frames without stretching them. `crop_to_rgb` converts one
rectangle only, for region crops. `luma_thumbnail` averages the brightness plane over small cells
and `LumaThumbnail::matches` compares two thumbnails block by block, so a duplicate frame is
found without any colour conversion; `crop_grey` copies a rectangle of brightness as full-range
grey for region signatures.

## Boundaries

- Depends on: `rayon` for the row tasks, `job_model` for the stream's tags, and
  `frame_queue::PooledBuffer` for a frame's samples.
- Used by: `video_frames`, the detection scan and the region source in `crates/stages/`, the
  localized video's colour (`stages::localize::colour`), and the detection benchmark in
  `tools/visual_validation/`.
- Rules: a picture with odd or zero dimensions, or a buffer of the wrong size, is refused rather
  than read past; padding rows are black; a rectangle outside the picture writes nothing.
