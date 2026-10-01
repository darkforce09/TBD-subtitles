//! The GPU runtime archives: CUDA 13.4, cuDNN 9.26, ONNX Runtime 1.28.2 and TensorRT 10.14, and
//! the CUDA 13.3 compiler pieces that build mistral.rs.
//!
//! **Role:** pin every runtime archive by URL, size and SHA-256, and name the folder under
//! `runtime/` each one unpacks into. CUDA and cuDNN hashes come from NVIDIA's `redistrib_*.json`
//! manifests (CUDA 13.4.2, cuDNN 9.26.0); ONNX Runtime's is the GitHub release digest; TensorRT's
//! size and hash are those nixpkgs records for NVIDIA's tarball, checked by the first download.
//!
//! **Position:** part of `manifest`, which re-exports every public name; read by `archive.rs`,
//! `archive_libraries.rs`, `cuda_runtime` and the AppImage builder.
//!
//! **Signals and state:** constants only.
//!
//! **Invariants:** every URL is https and every hash 64 lowercase hex digits; the TensorRT build is
//! the one ONNX Runtime 1.28.2's CUDA 13 provider is built against (10.x, for CUDA 13.0), because
//! the provider links `libnvinfer.so.10` and a TensorRT 11 library will not load.

use super::{PinnedArchive, PinnedLibraries};

/// The CUDA toolkit folder name under `runtime/`.
pub const CUDA_FOLDER: &str = "cuda-13.4";
/// The cuDNN folder name under `runtime/`.
pub const CUDNN_FOLDER: &str = "cudnn-9.26";
/// A CUDA 13.3 compiler, used only to build mistral.rs, whose build accepts toolkits up to 13.3;
/// the libraries it links against at run time are the 13.4 ones.
pub const CUDA_BUILD_FOLDER: &str = "cuda-13.3-build";

macro_rules! nv {
    ($id:literal, $to:expr, $path:literal, $size:literal, $sha:literal) => {
        PinnedArchive {
            id: $id,
            unpack_to: $to,
            url: concat!("https://developer.download.nvidia.com/compute/", $path),
            size: $size,
            sha256: $sha,
        }
    };
}

/// The CUDA 13 runtime libraries ONNX Runtime, ggml and candle load, and the compiler pieces the
/// ggml and candle kernels are built with.
pub const CUDA_ARCHIVES: &[PinnedArchive] = &[
    nv!(
        "cuda_cudart",
        CUDA_FOLDER,
        "cuda/redist/cuda_cudart/linux-x86_64/cuda_cudart-linux-x86_64-13.4.92-archive.tar.xz",
        1_704_640,
        "0ac5dbc538d04e9983bc493b410cce4b459e1ee9f5f6654b6464ef7b3e14a8b5"
    ),
    nv!(
        "libcublas",
        CUDA_FOLDER,
        "cuda/redist/libcublas/linux-x86_64/libcublas-linux-x86_64-13.8.0.4-archive.tar.xz",
        877_123_136,
        "bd6ffcce561672da6abb928214a6694f392741e7d1e2d7dfde0da4bd651706b6"
    ),
    nv!(
        "libcufft",
        CUDA_FOLDER,
        "cuda/redist/libcufft/linux-x86_64/libcufft-linux-x86_64-12.4.0.43-archive.tar.xz",
        249_327_988,
        "f7f8ac2cc67714989a3c8140cda39b1b7ed7d462a491e1ad3f53adb777f5070f"
    ),
    nv!(
        "libcurand",
        CUDA_FOLDER,
        "cuda/redist/libcurand/linux-x86_64/libcurand-linux-x86_64-10.4.4.72-archive.tar.xz",
        86_992_868,
        "6f725f9187dc5f675308b830f786c163f5b1b499d86986f94fb60aa701c13c0b"
    ),
    nv!(
        "cuda_nvrtc",
        CUDA_FOLDER,
        "cuda/redist/cuda_nvrtc/linux-x86_64/cuda_nvrtc-linux-x86_64-13.4.92-archive.tar.xz",
        72_311_456,
        "defc747cfa5953b86f4c67414fc349d21149e9d513c096cb89c64b8ec4c0c932"
    ),
    nv!(
        "libnvjitlink",
        CUDA_FOLDER,
        "cuda/redist/libnvjitlink/linux-x86_64/libnvjitlink-linux-x86_64-13.4.92-archive.tar.xz",
        58_209_588,
        "e24557473e5e2e1dc591209558523deaca48b32123fe2b822f994b8e691c8ae3"
    ),
    nv!(
        "cuda_nvcc",
        CUDA_FOLDER,
        "cuda/redist/cuda_nvcc/linux-x86_64/cuda_nvcc-linux-x86_64-13.4.92-archive.tar.xz",
        33_304_524,
        "60998f40cc9df5826b2a846e2dcc328133fbc5ef539398ce0a21cf5dcac7193d"
    ),
    nv!(
        "cuda_crt",
        CUDA_FOLDER,
        "cuda/redist/cuda_crt/linux-x86_64/cuda_crt-linux-x86_64-13.4.92-archive.tar.xz",
        101_980,
        "c969e61ded12dbf0cf20ace72867f1622ec0e8ea1e1d305e27761012025f7aaa"
    ),
    nv!(
        "libnvvm",
        CUDA_FOLDER,
        "cuda/redist/libnvvm/linux-x86_64/libnvvm-linux-x86_64-13.4.92-archive.tar.xz",
        51_321_404,
        "0e619cf3d5b27f7c812665599ca9761fc296e56129b24a85d822a5d8a1a3bd75"
    ),
    nv!(
        "cccl",
        CUDA_FOLDER,
        "cuda/redist/cccl/linux-x86_64/cccl-linux-x86_64-13.3.4.3.1-archive.tar.xz",
        1_404_324,
        "6b7516074f42f80dd3feb9c11318c8e1c5ab56cdd9f18fff4c5ace2e7e0a8209"
    ),
    nv!(
        "cuda_culibos",
        CUDA_FOLDER,
        "cuda/redist/cuda_culibos/linux-x86_64/cuda_culibos-linux-x86_64-13.4.92-archive.tar.xz",
        21_440,
        "241c618719e0613ce73b781c4e84b5eccf04e5baa3bae57de07980edb68a0e55"
    ),
    nv!(
        "cuda_profiler_api",
        CUDA_FOLDER,
        "cuda/redist/cuda_profiler_api/linux-x86_64/cuda_profiler_api-linux-x86_64-13.4.92-archive.tar.xz",
        17_112,
        "9d405e9b0fd9a5c29e680d6f6750b9ca79b7df91e253d4dffea3495057a75f08"
    ),
    nv!(
        "cudnn",
        CUDNN_FOLDER,
        "cudnn/redist/cudnn/linux-x86_64/cudnn-linux-x86_64-9.26.0.51_cuda13-archive.tar.xz",
        909_961_620,
        "e62c9b4af62ea130765ea5b13244c05c9452edf1690072373d193d19766d9850"
    ),
];

/// The CUDA 13.3.1 compiler pieces unpacked into `CUDA_BUILD_FOLDER`.
pub const CUDA_BUILD_ARCHIVES: &[PinnedArchive] = &[
    nv!(
        "cuda_nvcc-13.3",
        CUDA_BUILD_FOLDER,
        "cuda/redist/cuda_nvcc/linux-x86_64/cuda_nvcc-linux-x86_64-13.3.73-archive.tar.xz",
        31_628_824,
        "2ff9f9954060794a1c5134a933ccb45bec723d866b2629dadfe4a1a313f21068"
    ),
    nv!(
        "cuda_crt-13.3",
        CUDA_BUILD_FOLDER,
        "cuda/redist/cuda_crt/linux-x86_64/cuda_crt-linux-x86_64-13.3.73-archive.tar.xz",
        99_152,
        "1251aa9d668c607a103489cd2250773701e83a313e355578044622cf36713a9d"
    ),
    nv!(
        "libnvvm-13.3",
        CUDA_BUILD_FOLDER,
        "cuda/redist/libnvvm/linux-x86_64/libnvvm-linux-x86_64-13.3.73-archive.tar.xz",
        49_508_948,
        "206b1ab4979c09b5c32f8bf907c42bc9e16cd7454cf6036f524c45a58d060f93"
    ),
    nv!(
        "cuda_cudart-13.3",
        CUDA_BUILD_FOLDER,
        "cuda/redist/cuda_cudart/linux-x86_64/cuda_cudart-linux-x86_64-13.3.29-archive.tar.xz",
        1_573_744,
        "1e59c4888267d27ba1a9bd0f3669a6439db1334a96e754cd9013c7c73e18dc9d"
    ),
    nv!(
        "cccl-13.3",
        CUDA_BUILD_FOLDER,
        "cuda/redist/cccl/linux-x86_64/cccl-linux-x86_64-13.3.3.4.1-archive.tar.xz",
        1_262_552,
        "26957cede74f9341174ecaf0372f3f886e7c46ceccb98d6dc775fe2b68d19268"
    ),
];

/// The ONNX Runtime folder name under `runtime/`.
pub const ONNX_RUNTIME_FOLDER: &str = "onnxruntime-1.28.2";

/// Microsoft's ONNX Runtime 1.28.2 built for CUDA 13, loaded by the `ort` crate at run time. The
/// hash is the release asset's digest on GitHub.
pub const ONNX_RUNTIME_ARCHIVE: PinnedArchive = PinnedArchive {
    id: "onnxruntime",
    unpack_to: ONNX_RUNTIME_FOLDER,
    url: "https://github.com/microsoft/onnxruntime/releases/download/v1.28.2/onnxruntime-linux-x64-gpu_cuda13-1.28.2.tgz",
    size: 240_868_705,
    sha256: "118ca8dbc4e4bb9b3b7fea137d796a89d957c9aa70e1dc3a5199a302cdd5bb32",
};

/// The TensorRT folder name under `runtime/`.
pub const TENSORRT_FOLDER: &str = "tensorrt-10.14";

/// The TensorRT version ONNX Runtime 1.28.2's TensorRT provider is built against; the engine
/// cache key's TensorRT part.
pub const TENSORRT_VERSION: &str = "10.14.1.48";

/// NVIDIA's TensorRT 10.14.1.48 tarball for CUDA 13.0 on x86-64 Linux. About 7 GB; only
/// [`TENSORRT_LIBRARIES`] are kept from it.
pub const TENSORRT_ARCHIVE: PinnedArchive = PinnedArchive {
    id: "tensorrt",
    unpack_to: TENSORRT_FOLDER,
    url: "https://developer.nvidia.com/downloads/compute/machine-learning/tensorrt/10.14.1/tars/TensorRT-10.14.1.48.Linux.x86_64-gnu.cuda-13.0.tar.gz",
    size: 7_004_299_092,
    sha256: "c74af67db57f1a0d7e66bb01ab93f1ecda5facac491ca76e680d832f1e035ce6",
};

/// The TensorRT libraries ONNX Runtime's TensorRT provider loads: the two it links
/// (`libnvinfer`, `libnvonnxparser`), the plugin library it opens by name, and the builder
/// resources the builder opens. The Windows builder resource, which only builds engines for
/// Windows, is left out.
pub const TENSORRT_LIBRARIES: PinnedLibraries = PinnedLibraries {
    archive: TENSORRT_ARCHIVE,
    prefixes: &[
        "libnvinfer.so",
        "libnvinfer_plugin.so",
        "libnvonnxparser.so",
        "libnvinfer_builder_resource",
    ],
    skipped: &["_win."],
};

/// Every runtime archive the Settings page and the stack spike's `fetch` download: the CUDA
/// libraries, ONNX Runtime, then the CUDA 13.3 build pieces. TensorRT is not among them: only the
/// AppImage builder fetches it, through [`TENSORRT_LIBRARIES`].
pub fn runtime_archives() -> impl Iterator<Item = &'static PinnedArchive> {
    CUDA_ARCHIVES
        .iter()
        .chain(std::iter::once(&ONNX_RUNTIME_ARCHIVE))
        .chain(CUDA_BUILD_ARCHIVES)
}

#[cfg(test)]
#[path = "tests/gpu_runtime.rs"]
mod tests;
