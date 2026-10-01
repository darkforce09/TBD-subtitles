//! ONNX Runtime sessions on the CUDA execution provider.
//!
//! **Role:** open an ONNX model on the GPU, refusing a silent fall-back to the CPU, and describe
//! its inputs and outputs for logs and checks.
//!
//! **Position:** called by every ONNX backend in this folder; uses the `ort` crate, whose
//! prebuilt ONNX Runtime 1.28 loads the CUDA 13 libraries from the worker's `LD_LIBRARY_PATH`.
//!
//! **Signals and state:** one ONNX Runtime environment per process, created on first use.
//!
//! **Invariants:** a CUDA session either registers the CUDA provider or fails; it never runs on
//! the CPU while claiming the GPU. Each CUDA session's memory arena stays within
//! [`SESSION_MEMORY_LIMIT`] and grows by what is requested, so a worker, with its CUDA context,
//! stays within the 5.5 GB VRAM cap per GPU worker whatever the card has free.

use std::fmt;
use std::path::Path;

use ort::ep::{self, ArenaExtendStrategy};
use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;

/// Why an ONNX model could not be opened or run.
#[derive(Debug)]
pub struct OnnxError {
    pub context: String,
    pub message: String,
}

impl OnnxError {
    pub fn new(context: impl Into<String>, message: impl fmt::Display) -> OnnxError {
        OnnxError {
            context: context.into(),
            message: message.to_string(),
        }
    }
}

impl fmt::Display for OnnxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.context, self.message)
    }
}

impl std::error::Error for OnnxError {}

/// Where a session runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    Cuda,
    Cpu,
}

/// The most VRAM one CUDA session's memory arena may take: 4.5 GiB. The largest measured model,
/// separation, peaks near 4.3 GB; with the CUDA context the worker stays within its 5.5 GB cap.
pub const SESSION_MEMORY_LIMIT: usize = 4608 * 1024 * 1024;

/// The CUDA execution provider every CUDA session registers.
///
/// The arena is capped at [`SESSION_MEMORY_LIMIT`] and grows by what each allocation requests
/// rather than by powers of two. The cuDNN convolution search keeps ONNX Runtime's exhaustive
/// default and its maximum workspace: the limit bounds the workspace the search may take, so the
/// same algorithms are chosen whenever they fit, and the choice no longer depends on how much
/// VRAM happens to be free.
pub fn cuda_provider() -> ep::CUDA {
    ep::CUDA::default()
        .with_memory_limit(SESSION_MEMORY_LIMIT)
        .with_arena_extend_strategy(ArenaExtendStrategy::SameAsRequested)
}

/// Open `model` on `device` with full graph optimisation.
pub fn open(model: &Path, device: Device) -> Result<Session, OnnxError> {
    let context = || format!("opening {}", model.display());
    let mut builder = Session::builder()
        .map_err(|e| OnnxError::new(context(), e))?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| OnnxError::new(context(), e))?;
    if device == Device::Cuda {
        builder = builder
            .with_execution_providers([cuda_provider().build().error_on_failure()])
            .map_err(|e| OnnxError::new(context(), e))?;
    }
    builder
        .commit_from_file(model)
        .map_err(|e| OnnxError::new(context(), e))
}

/// One line per input and output: name and type, as ONNX Runtime reports them.
pub fn describe(session: &Session) -> Vec<String> {
    let inputs = session
        .inputs()
        .iter()
        .map(|o| format!("input {}: {:?}", o.name(), o.dtype()));
    let outputs = session
        .outputs()
        .iter()
        .map(|o| format!("output {}: {:?}", o.name(), o.dtype()));
    inputs.chain(outputs).collect()
}

#[cfg(test)]
#[path = "tests/session.rs"]
mod tests;
