# Model store manifest: GPU runtime archives

The GPU runtime archives the model store downloads, pinned by URL, size and SHA-256: CUDA 13.4,
cuDNN 9.26, ONNX Runtime 1.28.2, TensorRT 10.14 and the CUDA 13.3 compiler pieces. The parent
module `../manifest.rs` holds the pin types and the model files, and re-exports every public name
here.

## Contents

```text
crates/inference/src/model_store/manifest/
├── gpu_runtime.rs  the runtime archives, their folder names, TensorRT's pin, version and library selection
└── tests/          unit tests for the TensorRT pin, its folder name, its selection and its marker
```

## How it works

`../manifest.rs` declares `PinnedFile`, `PinnedArchive` and `PinnedLibraries` and re-exports
`gpu_runtime.rs`, so callers keep naming `manifest::CUDA_FOLDER`, `manifest::CUDA_BUILD_ARCHIVES`
and the rest. The sources of the hashes:

- CUDA and cuDNN: NVIDIA's `redistrib_13.4.2.json` and `redistrib_9.26.0.json`, and the CUDA
  13.3.1 compiler pieces that build mistral.rs into `cuda-13.3-build/`;
- ONNX Runtime: the GitHub release digest of `onnxruntime-linux-x64-gpu_cuda13-1.28.2.tgz`;
- TensorRT: `TensorRT-10.14.1.48.Linux.x86_64-gnu.cuda-13.0.tar.gz`, 7,004,299,092 bytes, the size
  and hash nixpkgs records for it. NVIDIA's download host is not reachable from the build
  container, so the host's first `cargo appimage` is the check: a different size or hash fails
  the install and keeps nothing.

`TENSORRT_LIBRARIES` keeps the libraries ONNX Runtime's TensorRT provider loads: `libnvinfer`
and `libnvonnxparser`, which it links, `libnvinfer_plugin`, which it opens by name, and the
`libnvinfer_builder_resource` libraries, which the engine builder opens; the Windows builder
resource and every static archive are left out. TensorRT 10.14 is the version ONNX Runtime
1.28.2's CUDA 13 build is made against; its provider links `libnvinfer.so.10`, so TensorRT 11
does not load. `runtime_archives` lists what Settings and the stack spike download; TensorRT is
not among them.

## Boundaries

- Depends on: `../manifest.rs` for the pin types; constants only.
- Used by: `archive.rs`, `archive_libraries.rs` and `mod.rs` of the model store,
  `crates/inference/src/cuda_runtime/` (the folder names, the TensorRT version), the app's model
  downloads and `tools/appimage_builder/`, all through the `manifest` re-exports.
- Rules:
  - every URL is https and every hash 64 lowercase hex digits
    (`every_pin_is_https_with_a_sha256` in `../tests/model_store.rs`);
  - NVIDIA's CUDA archives are `.tar.xz` on `developer.download.nvidia.com`; TensorRT's is a
    `.tar.gz` on `developer.nvidia.com` (`tensorrt_is_nvidias_10_14_tarball_for_cuda_13`);
  - TensorRT is no Settings download (`tensorrt_is_not_a_settings_download`).

## Related documentation

- [Decisions: inference engines](/documentation/decisions/inference_engines.md) — why TensorRT is
  pinned and bundled, and its licence.
- [Building the AppImage](/documentation/runbooks/building_the_appimage.md) — where the TensorRT
  download happens.
