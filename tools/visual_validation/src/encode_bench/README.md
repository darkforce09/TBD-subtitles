# Localized-video encode benchmark

The `encode-bench` command: times, on one clip of a video, the encodes the localized video can
run — the H.264 segment encode matching the source with x264 and NVENC presets, and the
whole-video HEVC encode with NVENC presets — and prints their speed, size and quality against
the source as Markdown tables.

## Contents

```text
tools/visual_validation/src/encode_bench/
├── args.rs     the clip's decode, the whole-video HEVC encode and the PSNR comparison command lines
├── measure.rs  one encode streamed through the tool, timed, sized and compared; the table rows
├── mod.rs      the command's options, the encoder checks, the segment rows and the order
└── tests/      options, command lines, the PSNR line, segment specs and table formatting
```

## How it works

`mod.rs` probes the video, reads its H.264 stream with `probe_h264_source` and checks which
NVENC encoders run (`segment_encoder` for `h264_nvenc`, `available_encoder` for `hevc_nvenc`;
each is a one-frame test encode), and prints a header with the clip, the frame format and the
encoders found.

Every row runs the same way (`measure.rs`): FFmpeg decodes the clip (`-ss`, `-t`, the first
video stream, no frame-rate conversion) into raw frames of the format the localize stage blends
in (`stages::localize::frame_format`: 10-bit 4:2:0 for a 10-bit source, else 8-bit), the tool
copies them from that pipe, enlarged to 1 MiB as the stage's decoder does, into the encoder's
stdin, and the clock runs from the encoder's start to its exit. The frame rate counts the whole
frames that reached the encoder; the size is the encoded file's; a third FFmpeg compares the
encoded clip with the source's clip frame by frame, each picture stamped with its index (Matroska
rounds times to the millisecond, so pairing by time would match neighbouring frames), through the
`psnr` filter, and the row reports its mean.

The segment rows take the production command line: `SegmentSpec::for_source` for the source and
encoder, with only `preset` changed, through `segment_args` — x264 at ultrafast, veryfast,
faster, fast, medium, slow, slower and veryslow (production: `X264_SEGMENT_PRESET`), then NVENC
at p1 to p7 (production: `NVENC_SEGMENT_PRESET`). The whole-video rows run NVENC HEVC with the
production settings (`-tune hq -rc vbr -cq 19 -b:v 0`, `main` or `main10`, the peak rate at 1.25
times the source's) at p1 to p7 (production: `HEVC_NVENC_PRESET`), video alone. A row whose encoder does not run
says `unavailable`; a source the segment encode cannot match (not H.264, a profile or level it
does not know, or NVENC for a 10-bit source) says `n/a` and why; a failed encode prints its error.

## Boundaries

- Depends on: `crates/media_io` (`encode::segments`, `encode::available_encoder`, `probe`,
  `video_frames::{PixelFormat, pipe}`), `crates/stages` (`localize::frame_format`),
  `crates/job_model` (`LocalizedEncoder`), `crates/child_process`; the crates.io crates `clap`
  and `anyhow`; FFmpeg.
- Used by: `tools/visual_validation/src/main.rs` (the `encode-bench` command).
- Rules: the source video is only read; the encoded clips are written only to `--out-dir` or a
  temporary folder the command removes; a segment row changes nothing but the preset of the
  production command line
  (`a_segment_row_runs_the_production_command_with_only_its_preset_changed`).

## Related documentation

- [Localized video decisions](/documentation/decisions/localized_video.md) — the segment encode
  and the presets this benchmark measures.
- [Video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md) — the
  localized video these encodes write.
