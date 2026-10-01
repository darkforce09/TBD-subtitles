# Bundled GPU runtime

The CUDA 13.4, cuDNN 9.26, ONNX Runtime 1.28.2 and TensorRT 10.14 libraries the AppImage carries
in `usr/bin/cuda/<folder>/lib`, the place the app's CUDA locator looks first.

## Contents

```text
tools/appimage_builder/src/gpu_runtime/
├── mod.rs  `ensure_unpacked` for the pinned archives and TensorRT, and `bundle`: required, run-time loaded, providers, closure
└── tests/  a fake runtime of tiny ELF libraries: the folder list, the TensorRT bundle, a missing plugin library
```

## How it works

`ensure_unpacked` installs any pinned CUDA or ONNX Runtime archive the user's runtime folder
lacks, and the CUDA 13.3 compiler archives the local worker's build uses, through
`inference::model_store`. It then installs TensorRT's libraries with
`model_store::install_libraries`: the 7 GB tarball is read once (from
`runtime/.archives/TensorRT-10.14.1.48.Linux.x86_64-gnu.cuda-13.0.tar.gz` when it was placed there
by hand, else from NVIDIA's URL), hashed while it streams, and only the libraries the provider
loads land in `runtime/tensorrt-10.14/lib`; a size or hash mismatch keeps nothing and fails the
step.

`bundle` walks four folders. From each it takes the libraries `inference::cuda_runtime` requires,
the libraries loaded with `dlopen` (NVRTC's builtins, nvJitLink, every cuDNN library, TensorRT's
`libnvinfer_plugin.so.*` and `libnvinfer_builder_resource*`), and ONNX Runtime's TensorRT
provider `libonnxruntime_providers_tensorrt.so`, which ONNX Runtime opens by file name. It walks
their NEEDED closure inside the four runtime folders, copies each library with its soname symlinks
into the matching bundled folder, asks `CudaRuntime::locate` to find the bundle with TensorRT
available, and returns every bundled name so the workers' library walks leave them out.

## Boundaries

- Depends on: `inference::cuda_runtime` (the required library lists, TensorRT's run-time loaded
  prefixes and provider name, and `CudaRuntime`), `inference::model_store` (the archives, the
  TensorRT selection, their installs and the runtime folder), `elf`.
- Used by: `main.rs`, before the build (nvcc) and while laying out the AppDir; `ffmpeg` and
  `runtime` use `print_progress`.
- Rules:
  - nvcc, headers and static archives are never copied, only `*.so*` files the walk reaches;
  - the bundle passes `CudaRuntime::locate` with TensorRT available before the run goes on
    (`a_bundle_without_the_plugin_library_fails_naming_it`);
  - the TensorRT libraries other than `libnvinfer` and `libnvinfer_plugin` are bundled for the
    owner's own use only (the licence note in the
    [inference engines decision](/documentation/decisions/inference_engines.md#2026-10-01--tensorrt-runs-the-pp-ocrv5-detectors)).
