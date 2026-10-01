//! Where the CUDA 13 runtime libraries, ONNX Runtime and TensorRT live, and the environment a GPU
//! worker needs to load them.
//!
//! **Role:** find the folder holding cudart, cuBLAS, cuFFT, cuRAND, NVRTC and cuDNN for CUDA 13,
//! check the libraries ONNX Runtime's CUDA provider loads are all there, note whether a complete
//! TensorRT 10.14 lies beside them for ONNX Runtime's TensorRT provider, and give the
//! `LD_LIBRARY_PATH` and `ORT_DYLIB_PATH` values that make ORT, ggml-cuda and candle find them.
//!
//! **Position:** called by whoever starts a GPU worker (the job runner, the stack spike and
//! visual validation tools) before the spawn, and by the app's settings and the AppImage builder
//! to check a runtime; reads the folder `model_store` fills.
//!
//! **Signals and state:** reads `LD_LIBRARY_PATH` and the file system; holds nothing.
//!
//! **Invariants:** a runtime is returned only when every required library file exists; TensorRT
//! is optional and never decides whether a runtime is found; the packaged folder beside the
//! executable wins over the user folder; TensorRT's library folder is on the worker's library
//! path whenever it is found, because ONNX Runtime's provider opens `libnvinfer_plugin.so.10` by
//! bare name.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::model_store::manifest::{
    CUDA_FOLDER, CUDNN_FOLDER, ONNX_RUNTIME_FOLDER, TENSORRT_FOLDER, TENSORRT_VERSION,
};

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

/// ONNX Runtime's TensorRT provider, which ONNX Runtime opens from its own folder when a session
/// asks for TensorRT.
pub const TENSORRT_PROVIDER_LIB: &str = "libonnxruntime_providers_tensorrt.so";

/// The TensorRT libraries the TensorRT provider links, by soname.
pub const REQUIRED_TENSORRT_LIBS: &[&str] = &["libnvinfer.so.10", "libnvonnxparser.so.10"];

/// Name prefixes of the TensorRT libraries opened at run time, which no NEEDED entry names: the
/// plugin library the provider opens by bare name, and the builder's resource libraries.
pub const TENSORRT_LOADED_AT_RUN_TIME: &[&str] =
    &["libnvinfer_plugin.so.", "libnvinfer_builder_resource"];

/// A CUDA 13 runtime found on disk.
#[derive(Debug, Clone)]
pub struct CudaRuntime {
    /// The toolkit root: `lib/`, `include/`, `bin/nvcc`.
    pub cuda_root: PathBuf,
    /// The cuDNN root: `lib/`, `include/`.
    pub cudnn_root: PathBuf,
    /// The ONNX Runtime root: `lib/`, `include/`.
    pub onnxruntime_root: PathBuf,
    /// The TensorRT root (`lib/`), when a complete TensorRT lies beside the runtime and ONNX
    /// Runtime's TensorRT provider is in its folder; `None` otherwise.
    pub tensorrt_root: Option<PathBuf>,
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
    /// Look in `<exe dir>/cuda/` first, then in `runtime_dir`. TensorRT is taken from the same
    /// folder as the runtime, when it is complete there.
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
            let mut runtime = CudaRuntime {
                cuda_root: base.join(CUDA_FOLDER),
                cudnn_root: base.join(CUDNN_FOLDER),
                onnxruntime_root: base.join(ONNX_RUNTIME_FOLDER),
                tensorrt_root: None,
            };
            match runtime.first_missing() {
                None => {
                    let tensorrt_root = base.join(TENSORRT_FOLDER);
                    if runtime.first_missing_tensorrt(&tensorrt_root).is_none() {
                        runtime.tensorrt_root = Some(tensorrt_root);
                    }
                    return Ok(runtime);
                }
                Some(missing) => looked_in.push((base, missing)),
            }
        }
        Err(MissingRuntime { looked_in })
    }

    /// The first required library not on disk, if any. TensorRT is not required.
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

    /// What keeps TensorRT at `tensorrt_root` from being used with this runtime, if anything:
    /// ONNX Runtime's TensorRT provider, a linked TensorRT library, or a library opened at run
    /// time, named by the first one missing.
    pub fn first_missing_tensorrt(&self, tensorrt_root: &Path) -> Option<String> {
        let provider = self
            .onnxruntime_root
            .join("lib")
            .join(TENSORRT_PROVIDER_LIB);
        if !provider.exists() {
            return Some(provider.display().to_string());
        }
        let lib = tensorrt_root.join("lib");
        if let Some(missing) = REQUIRED_TENSORRT_LIBS
            .iter()
            .map(|name| lib.join(name))
            .find(|path| !path.exists())
        {
            return Some(missing.display().to_string());
        }
        let names: Vec<String> = std::fs::read_dir(&lib)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        TENSORRT_LOADED_AT_RUN_TIME
            .iter()
            .find(|prefix| !names.iter().any(|name| name.starts_with(*prefix)))
            .map(|prefix| format!("{}/{prefix}*", lib.display()))
    }

    /// Whether ONNX Runtime's TensorRT provider can run with this runtime.
    pub fn tensorrt_available(&self) -> bool {
        self.tensorrt_root.is_some()
    }

    /// The TensorRT version found, for engine cache keys and reports; `None` without TensorRT.
    pub fn tensorrt_version(&self) -> Option<&'static str> {
        self.tensorrt_available().then_some(TENSORRT_VERSION)
    }

    /// The library folders: CUDA, cuDNN, ONNX Runtime, then TensorRT when it was found.
    pub fn lib_dirs(&self) -> Vec<PathBuf> {
        [&self.cuda_root, &self.cudnn_root, &self.onnxruntime_root]
            .into_iter()
            .chain(self.tensorrt_root.as_ref())
            .map(|root| root.join("lib"))
            .collect()
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
