# CUDA runtime

Finds the CUDA 13 and cuDNN 9 libraries and ONNX Runtime the GPU backends load, notes whether
TensorRT 10.14 lies beside them for ONNX Runtime's TensorRT provider, and gives the environment a
GPU worker is started with (`LD_LIBRARY_PATH`, `ORT_DYLIB_PATH`) so ONNX Runtime, ggml and candle
all load the same libraries.

## Contents

```text
crates/inference/src/cuda_runtime/
├── mod.rs  the runtime lookup, the required library lists, the optional TensorRT and the worker environment
└── tests/  unit tests for the lookup order, a missing library, TensorRT found or not, and the library path
```

## How it works

`CudaRuntime::locate` looks in `<exe dir>/cuda/` and then in the user runtime folder, and returns
the first folder holding every library of `REQUIRED_CUDA_LIBS`, `REQUIRED_CUDNN_LIBS` and
`REQUIRED_ONNX_RUNTIME_LIBS`. In that same folder it then looks for `tensorrt-10.14/`:
`tensorrt_root` is set only when `REQUIRED_TENSORRT_LIBS` (`libnvinfer.so.10`,
`libnvonnxparser.so.10`) are there, a library starts with each of `TENSORRT_LOADED_AT_RUN_TIME`
(`libnvinfer_plugin.so.`, `libnvinfer_builder_resource`), and ONNX Runtime's folder holds
`libonnxruntime_providers_tensorrt.so`. A missing TensorRT never fails the lookup;
`first_missing_tensorrt` names what is missing, `tensorrt_available` and `tensorrt_version`
(`10.14.1.48`) answer callers that choose the detector engine or key its engine cache.
`lib_dirs` and `library_path` add TensorRT's `lib/` after ONNX Runtime's when it was found, because
the provider opens `libnvinfer_plugin.so.10` by bare name.

The AppImage carries TensorRT in `usr/bin/cuda/tensorrt-10.14/lib`. Outside the AppImage, the user
runtime folder has TensorRT only after `cargo appimage` installed it there; Settings does not
download it.

## Boundaries

- Depends on: `crates/inference/src/model_store/manifest.rs` for the folder names `cuda-13.4`,
  `cudnn-9.26`, `onnxruntime-1.28.2` and `tensorrt-10.14`, and the TensorRT version.
- Used by:
  - `crates/pipeline/src/runner/mod.rs`, `tools/stack_spike/` and
    `tools/visual_validation/src/pilot.rs`, which start every GPU worker with
    `CudaRuntime::worker_env`; `crates/pipeline/src/workers/mod.rs` reads `REQUIRED_CUDA_LIBS`;
  - the app's settings (`apps/tbd_subtitles/src/settings/services/system_check.rs` and
    `model_downloads.rs`), which check that a runtime is found;
  - `tools/appimage_builder/src/gpu_runtime/mod.rs`, which bundles the libraries these lists name
    and checks the packaged `cuda/` folder, TensorRT included.
- Rules:
  - a runtime is returned only when every library in `REQUIRED_CUDA_LIBS`,
    `REQUIRED_CUDNN_LIBS` and `REQUIRED_ONNX_RUNTIME_LIBS` exists (`a_missing_library_is_named`);
  - TensorRT is optional and used only when complete (`tensorrt_is_optional`,
    `tensorrt_without_a_library_it_loads_is_not_used`);
  - `<exe dir>/cuda/` wins over the user runtime folder (`the_packaged_folder_wins`);
  - the runtime folders come first in `LD_LIBRARY_PATH`, TensorRT's among them when found
    (`the_library_path_puts_the_runtime_first`,
    `a_complete_tensorrt_is_found_and_put_on_the_library_path`).

## Related documentation

- [Development environment](/documentation/runbooks/development_environment.md#cuda-libraries-for-onnx-runtime)
  — where the libraries live and how builds and workers find them.
- [TensorRT runs the PP-OCRv5 detectors](/documentation/decisions/inference_engines.md#2026-10-01--tensorrt-runs-the-pp-ocrv5-detectors)
  — why TensorRT lies beside the runtime.
