# Full-resolution screening benchmark

The `detect-bench` command: measures, on one clip of a video, what screening on-screen text at
full resolution would cost — decoding, colour conversion, the PP-OCRv5 mobile detector by path,
batch and worker count, the proxy baseline and the server detector — and prints Markdown tables.

## Contents

```text
tools/visual_validation/src/detect_bench/
├── decode.rs    four FFmpeg routes into a Rust reader, the conversion timing and the sample frames
├── detector.rs  the stock predictor and the padded DB-model path, each owning one CUDA session
├── mod.rs       the command's options, the CUDA runtime re-execution, the host line and the order
├── runs.rs      one measured detector configuration, and the screening and confirmation tables
├── table.rs     Markdown tables and optional figures
├── usage.rs     GPU and NVDEC utilisation through NVML and CPU ticks from /proc for one run
└── tests/       options, FFmpeg arguments, box clipping, ticks and table formatting
```

## How it works

`mod.rs` re-executes the binary once with the CUDA runtime's `LD_LIBRARY_PATH` and
`ORT_DYLIB_PATH` (`CudaRuntime::worker_env`, as the pipeline starts a GPU worker; the runtime is
`cuda/` beside the binary, `--runtime-dir`, or the app's runtime folder), probes the video and
prints the host line from the pipeline's `gpu_monitor`.

Section 1 (`decode.rs`) runs FFmpeg over the clip four times — rgb24 on the default 64 KiB pipe,
rgb24 and yuv420p with the pipe enlarged to 1 MiB on the read end (`F_SETPIPE_SZ`, never past
`pipe-max-size`), and NVDEC downloaded as nv12 — reading every frame into one reused buffer, with
`usage.rs` measuring wall time, CPU cores (this process and its reaped FFmpeg) and GPU and NVDEC
use. It then times `media_io::yuv`'s BT.709 limited-range conversion on the first yuv420p and
nv12 frames and checks the parallel bytes equal the scalar ones.

Sections 2 and 3 (`runs.rs`) hold every sample-step frame (round(fps / 2)) in memory, at full
resolution and as the production 640-wide proxy with the deblocking filter skipped, so decoding
stays out of the timing. Each configuration opens one detector per worker (`detector.rs`), warms
it on one batch, then starts the clock; batches go to workers round-robin and return reordered.
The stock path is oar-ocr's predictor at limit 1920, which stretches 1080 rows to 1088; the padded
path copies each frame into a reused 1088-row buffer with black rows below, normalizes the batch,
runs `DBModel::infer` once and post-processes each image on its own rayon task, clipping boxes
back to row 1080. Peak VRAM comes from the pipeline's `gpu_monitor::Monitor` over opening and
screening. Every session carries the production CUDA options with the `--memory-limit-mib` arena
limit; a failed configuration prints its error and runs once more at `--raised-limit-mib`. Each
path's fastest batch runs again with two workers; the proxy baseline is the stock predictor at
its default limit, batch 4. Section 3 times the server detector on 20 stills at limit 1920 and at
the default 960.

## Boundaries

- Depends on: `crates/inference` (`cuda_runtime`, `model_store`), `crates/media_io`
  (`Programs`, `yuv`, `probe`), `crates/pipeline` (`measure::gpu_monitor`,
  `measure::process_tree::clock_ticks`), `crates/child_process`; the crates.io crates
  `oar-ocr`, `ndarray`, `rayon`, `nvml-wrapper`, `image`, `clap` and `anyhow`; FFmpeg.
- Used by: `tools/visual_validation/src/main.rs` (the `detect-bench` command).
- Rules: the source video is only read; a failed configuration reports its error in its row
  rather than ending the bench; no box
  of the padded path reaches below the frame
  (`a_box_reaching_into_the_padding_is_clipped_to_the_frame`).

## Related documentation

- [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md) — the production
  detection the benchmark measures against.
