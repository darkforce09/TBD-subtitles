# Bundled GPU runtime

The CUDA 13.4, cuDNN 9.26 and ONNX Runtime 1.28.2 libraries the AppImage carries in
`usr/bin/cuda/<folder>/lib`, the place the app's CUDA locator looks first.

## Contents

```text
tools/appimage_builder/src/gpu_runtime/
└── mod.rs  `ensure_unpacked` for the pinned archives, and `bundle`: required libraries, run-time loaded ones, their closure
```

## How it works

`ensure_unpacked` installs any pinned CUDA or ONNX Runtime archive the user's runtime folder
lacks, and the CUDA 13.3 compiler archives the local worker's build uses, through
`inference::model_store`. `bundle` starts from the library lists
`inference::cuda_runtime` requires, adds the libraries loaded with `dlopen` (NVRTC's builtins,
nvJitLink and every cuDNN library), walks their NEEDED closure inside the three runtime folders, and copies each
library with its soname symlinks into the matching bundled folder. It then asks
`CudaRuntime::locate` to find the bundle, and returns every bundled name so the workers' library walks leave
them out.

## Boundaries

- Depends on: `inference::cuda_runtime` (the required library lists and `CudaRuntime`),
  `inference::model_store` (the archives, their install and the runtime folder), `elf`.
- Used by: `main.rs`, before the build (nvcc) and while laying out the AppDir; `ffmpeg` and
  `runtime` use `print_progress`.
- Rules:
  - nvcc, headers and static archives are never copied, only `*.so*` files the walk reaches;
  - the bundle passes `CudaRuntime::locate` before the run goes on.
