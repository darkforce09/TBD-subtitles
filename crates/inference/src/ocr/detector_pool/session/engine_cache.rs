//! Where a TensorRT engine is cached, and the shapes it is built for.
//!
//! **Role:** name the cache folder of one engine by a key over everything the engine depends
//! on, write the optimisation profile's shape string, size the builder's workspace within the
//! worker's memory cap, and tell whether a folder already holds a built engine.
//!
//! **Position:** used by `ort_detector.rs` when it opens a session on the TensorRT provider.
//!
//! **Signals and state:** reads the cache folder's listing; writes nothing itself (ONNX Runtime
//! writes the engine and timing caches into the folder).
//!
//! **Invariants:** the key changes whenever the GPU name, the driver, the TensorRT version, the
//! model file's SHA-256, the input shape or the precision changes, so a stale engine is never
//! reused; an identity with an empty field is refused rather than keyed.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::ocr::OcrError;
use crate::ocr::pool::EngineIdentity;

/// The folder under the app's data folder that holds one folder per engine key.
pub const TENSORRT_FOLDER: &str = "tensorrt";

/// The CUDA context and the runtime's own allocations, in MiB, left out of the sessions' share
/// of the worker's memory cap.
const CONTEXT_MIB: usize = 512;
/// The smallest workspace the builder is given, in MiB.
const MIN_WORKSPACE_MIB: usize = 256;

/// An error unless every field of `identity` is filled.
pub fn check_identity(identity: &EngineIdentity) -> Result<(), OcrError> {
    let fields = [
        ("GPU name", &identity.gpu_name),
        ("driver", &identity.driver),
        ("TensorRT version", &identity.tensorrt_version),
    ];
    match fields.iter().find(|(_, value)| value.trim().is_empty()) {
        Some((name, _)) => Err(format!(
            "the TensorRT engine cache needs the {name} to key its engines; it is unknown"
        )
        .into()),
        None => Ok(()),
    }
}

/// The cache key of an engine: 32 hexadecimal digits of the SHA-256 over every field it
/// depends on.
pub fn cache_key(
    identity: &EngineIdentity,
    model_sha256: &str,
    dims: [usize; 4],
    fp16: bool,
) -> String {
    let canonical = format!(
        "gpu={}\ndriver={}\ntensorrt={}\nmodel={}\nshape={}\nprecision={}\n",
        identity.gpu_name,
        identity.driver,
        identity.tensorrt_version,
        model_sha256,
        shape_text(dims),
        if fp16 { "fp16" } else { "fp32" }
    );
    hex(&Sha256::digest(canonical.as_bytes())[..16])
}

/// The SHA-256 of `bytes` in lowercase hexadecimal.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn shape_text(dims: [usize; 4]) -> String {
    dims.map(|dim| dim.to_string()).join("x")
}

/// The optimisation profile's shape for `input`, as TensorRT reads it: `x:4x3x1088x1920`.
/// The profile's minimum, optimum and maximum all take it, so the engine has one shape.
pub fn profile_shapes(input: &str, dims: [usize; 4]) -> String {
    format!("{input}:{}", shape_text(dims))
}

/// The workspace each of `sessions` sessions with a `pool_mib` arena may give the builder so
/// the worker stays within `cap_mib`.
pub fn workspace_mib(cap_mib: usize, sessions: usize, pool_mib: usize) -> usize {
    (cap_mib.saturating_sub(CONTEXT_MIB) / sessions.max(1))
        .saturating_sub(pool_mib)
        .max(MIN_WORKSPACE_MIB)
}

/// Whether `folder` holds a built engine.
pub fn holds_engine(folder: &Path) -> bool {
    std::fs::read_dir(folder).is_ok_and(|entries| {
        entries.flatten().any(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "engine")
        })
    })
}

#[cfg(test)]
#[path = "tests/engine_cache.rs"]
mod tests;
