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
//! the CPU while claiming the GPU.

use std::fmt;
use std::path::Path;

use ort::ep;
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

/// Open `model` on `device` with full graph optimisation.
pub fn open(model: &Path, device: Device) -> Result<Session, OnnxError> {
    let context = || format!("opening {}", model.display());
    let mut builder = Session::builder()
        .map_err(|e| OnnxError::new(context(), e))?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| OnnxError::new(context(), e))?;
    if device == Device::Cuda {
        builder = builder
            .with_execution_providers([ep::CUDA::default().build().error_on_failure()])
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
