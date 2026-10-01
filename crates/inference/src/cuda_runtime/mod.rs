//! Where the CUDA 13 runtime libraries and ONNX Runtime live, and the environment a GPU worker
//! needs to load them.
//!
//! **Role:** find the folder holding cudart, cuBLAS, cuFFT, cuRAND, NVRTC and cuDNN for CUDA 13,
//! check the libraries ONNX Runtime's CUDA provider loads are all there, and give the
//! `LD_LIBRARY_PATH` and `ORT_DYLIB_PATH` values that make ORT, ggml-cuda and candle find them.
//!
//! **Position:** called by whoever starts a GPU worker (the job runner, the stack spike and
//! visual validation tools) before the spawn, and by the app's settings and the AppImage builder
//! to check a runtime; reads the folder `model_store` fills.
//!
//! **Signals and state:** reads `LD_LIBRARY_PATH` and the file system; holds nothing.
//!
//! **Invariants:** a runtime is returned only when every required library file exists; the
//! packaged folder beside the executable wins over the user folder.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::model_store::manifest::{CUDA_FOLDER, CUDNN_FOLDER, ONNX_RUNTIME_FOLDER};

/// The libraries ONNX Runtime 1.28's CUDA provider and cuDNN load, by soname.
pub const REQUIRED_CUDA_LIBS: &[&str] = &[
    "libcudart.so.13",
    "libcublasLt.so.13",
    "libcublas.so.13",
    "libnvrtc.so.13",
    "libcurand.so.10",
    "libcufft.so.12",
];

/// The cuDNN libraries, by soname.
pub const REQUIRED_CUDNN_LIBS: &[&str] = &[
    "libcudnn.so.9",
    "libcudnn_graph.so.9",
    "libcudnn_ops.so.9",
    "libcudnn_cnn.so.9",
    "libcudnn_adv.so.9",
    "libcudnn_heuristic.so.9",
    "libcudnn_engines_precompiled.so.9",
    "libcudnn_engines_runtime_compiled.so.9",
];

/// The ONNX Runtime library `ort` loads, and the CUDA provider it loads beside it.
pub const REQUIRED_ONNX_RUNTIME_LIBS: &[&str] = &[
    "libonnxruntime.so",
    "libonnxruntime_providers_shared.so",
    "libonnxruntime_providers_cuda.so",
];

/// A CUDA 13 runtime found on disk.
#[derive(Debug, Clone)]
pub struct CudaRuntime {
    /// The toolkit root: `lib/`, `include/`, `bin/nvcc`.
    pub cuda_root: PathBuf,
    /// The cuDNN root: `lib/`, `include/`.
    pub cudnn_root: PathBuf,
    /// The ONNX Runtime root: `lib/`, `include/`.
    pub onnxruntime_root: PathBuf,
}

/// Why no runtime was found.
#[derive(Debug)]
pub struct MissingRuntime {
    /// Each folder looked in, with the first library missing there.
    pub looked_in: Vec<(PathBuf, String)>,
}

impl fmt::Display for MissingRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "no complete CUDA 13 runtime found")?;
        for (dir, missing) in &self.looked_in {
            write!(f, "; {} lacks {missing}", dir.display())?;
        }
        Ok(())
    }
}

impl std::error::Error for MissingRuntime {}

impl CudaRuntime {
    /// Look in `<exe dir>/cuda/` first, then in `runtime_dir`.
    pub fn locate(
        exe_dir: Option<&Path>,
        runtime_dir: &Path,
    ) -> Result<CudaRuntime, MissingRuntime> {
        let mut looked_in = Vec::new();
        let candidates = exe_dir
            .map(|dir| dir.join("cuda"))
            .into_iter()
            .chain(std::iter::once(runtime_dir.to_path_buf()));
        for base in candidates {
            let runtime = CudaRuntime {
                cuda_root: base.join(CUDA_FOLDER),
                cudnn_root: base.join(CUDNN_FOLDER),
                onnxruntime_root: base.join(ONNX_RUNTIME_FOLDER),
            };
            match runtime.first_missing() {
                None => return Ok(runtime),
                Some(missing) => looked_in.push((base, missing)),
            }
        }
        Err(MissingRuntime { looked_in })
    }

    /// The first required library not on disk, if any.
    pub fn first_missing(&self) -> Option<String> {
        let cuda_lib = self.cuda_root.join("lib");
        let cudnn_lib = self.cudnn_root.join("lib");
        let ort_lib = self.onnxruntime_root.join("lib");
        REQUIRED_CUDA_LIBS
            .iter()
            .map(|lib| cuda_lib.join(lib))
            .chain(REQUIRED_CUDNN_LIBS.iter().map(|lib| cudnn_lib.join(lib)))
            .chain(
                REQUIRED_ONNX_RUNTIME_LIBS
                    .iter()
                    .map(|lib| ort_lib.join(lib)),
            )
            .find(|path| !path.exists())
            .map(|path| path.display().to_string())
    }

    /// The library folders: CUDA, cuDNN, ONNX Runtime.
    pub fn lib_dirs(&self) -> [PathBuf; 3] {
        [
            self.cuda_root.join("lib"),
            self.cudnn_root.join("lib"),
            self.onnxruntime_root.join("lib"),
        ]
    }

    /// The ONNX Runtime library `ort` loads at run time.
    pub fn onnxruntime_library(&self) -> PathBuf {
        self.onnxruntime_root.join("lib").join("libonnxruntime.so")
    }

    /// `LD_LIBRARY_PATH` for a GPU worker: the runtime folders ahead of `inherited`.
    pub fn library_path(&self, inherited: Option<&str>) -> String {
        let mut parts: Vec<String> = self
            .lib_dirs()
            .iter()
            .map(|dir| dir.display().to_string())
            .collect();
        if let Some(rest) = inherited.filter(|rest| !rest.is_empty()) {
            parts.push(rest.to_string());
        }
        parts.join(":")
    }

    /// The environment a GPU worker is started with: the library path, and `ORT_DYLIB_PATH`, which
    /// tells `ort` where ONNX Runtime is.
    pub fn worker_env(&self) -> Vec<(String, String)> {
        let inherited = std::env::var("LD_LIBRARY_PATH").ok();
        vec![
            (
                "LD_LIBRARY_PATH".to_string(),
                self.library_path(inherited.as_deref()),
            ),
            (
                "ORT_DYLIB_PATH".to_string(),
                self.onnxruntime_library().display().to_string(),
            ),
        ]
    }
}

#[cfg(test)]
#[path = "tests/cuda_runtime.rs"]
mod tests;
