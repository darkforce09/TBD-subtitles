//! Models run through ONNX Runtime (the `ort` crate) on CUDA: speech recognition, separation, CTC
//! alignment and sound events.

pub mod separation;
pub mod session;

pub use session::{Device, OnnxError};

#[cfg(test)]
#[path = "tests/models.rs"]
mod tests;
