# CUDA runtime

Finds the CUDA 13 and cuDNN 9 libraries the GPU backends load, and gives the environment a GPU
worker is started with so ONNX Runtime, ggml and candle all load the same libraries.

## Contents

```text
crates/inference/src/cuda_runtime/
├── mod.rs  the runtime lookup, the required library list and the worker environment
└── tests/  unit tests for the lookup order, a missing library and the library path
```

## Boundaries

- Depends on: `crates/inference/src/model_store/manifest.rs` for the folder names `cuda-13.4`
  and `cudnn-9.26`.
- Used by: `tools/stack_spike/`, which starts every GPU worker with `CudaRuntime::worker_env`.
- Rules:
  - a runtime is returned only when every library in `REQUIRED_CUDA_LIBS` and
    `REQUIRED_CUDNN_LIBS` exists (`a_missing_library_is_named`);
  - `<exe dir>/cuda/` wins over the user runtime folder (`the_packaged_folder_wins`);
  - the runtime folders come first in `LD_LIBRARY_PATH`
    (`the_library_path_puts_the_runtime_first`).

## Related documentation

- [Development environment](/documentation/runbooks/development_environment.md#cuda-libraries-for-onnx-runtime)
  — where the libraries live and how builds and workers find them.
