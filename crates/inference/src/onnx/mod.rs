//! Models run through ONNX Runtime (the `ort` crate) on CUDA: speech recognition, separation, CTC
//! alignment, sound events and
//! inpainting.

pub mod ced;
pub mod lama;
pub mod parakeet_ctc;
pub mod parakeet_tdt;
pub mod separation;
pub mod session;

pub use session::{Device, OnnxError};

#[cfg(test)]
#[path = "tests/models.rs"]
mod tests;
