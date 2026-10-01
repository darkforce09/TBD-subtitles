# CUDA runtime

Finds the CUDA 13 and cuDNN 9 libraries and ONNX Runtime the GPU backends load, and gives the
environment a GPU worker is started with (`LD_LIBRARY_PATH`, `ORT_DYLIB_PATH`) so ONNX Runtime,
ggml and candle all load the same libraries.

## Contents

```text
crates/inference/src/cuda_runtime/
├── mod.rs  the runtime lookup, the required library list and the worker environment
└── tests/  unit tests for the lookup order, a missing library and the library path
```

## Boundaries

- Depends on: `crates/inference/src/model_store/manifest.rs` for the folder names `cuda-13.4`,
  `cudnn-9.26` and `onnxruntime-1.28.2`.
- Used by:
  - `crates/pipeline/src/runner/mod.rs`, `tools/stack_spike/` and
    `tools/visual_validation/src/pilot.rs`, which start every GPU worker with
    `CudaRuntime::worker_env`; `crates/pipeline/src/workers/mod.rs` reads `REQUIRED_CUDA_LIBS`;
  - the app's settings (`apps/tbd_subtitles/src/settings/services/system_check.rs` and
    `model_downloads.rs`), which check that a runtime is found;
  - `tools/appimage_builder/src/gpu_runtime/mod.rs`, which checks the packaged `cuda/` folder.
- Rules:
  - a runtime is returned only when every library in `REQUIRED_CUDA_LIBS`,
    `REQUIRED_CUDNN_LIBS` and `REQUIRED_ONNX_RUNTIME_LIBS` exists (`a_missing_library_is_named`);
  - `<exe dir>/cuda/` wins over the user runtime folder (`the_packaged_folder_wins`);
  - the runtime folders come first in `LD_LIBRARY_PATH`
    (`the_library_path_puts_the_runtime_first`).

## Related documentation

- [Development environment](/documentation/runbooks/development_environment.md#cuda-libraries-for-onnx-runtime)
  — where the libraries live and how builds and workers find them.
