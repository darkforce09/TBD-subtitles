//! The inference backends.
//!
//! **Role:** runs the models the GPU stages need behind one trait per capability: ONNX Runtime,
//! ggml and candle models, the language-model backends, the model store that downloads and
//! verifies model files and the CUDA runtime archives, and the lookup of that runtime.
//!
//! **Position:** called by `stages`, inside a `worker` process for every GPU stage; depends on
//! `job_model` and runs the `claude` CLI through `child_process`.
//!
//! **Signals and state:** reads model files from the models folder; loads each model once per
//! worker process.
//!
//! **Invariants:** models are downloaded already exported, from pinned URLs with checksums, and
//! never converted; no two ggml-bundling crates link into one binary.

pub mod candle;
pub mod cuda_runtime;
pub mod ggml;
pub mod llm;
pub mod model_store;
pub mod ocr;
pub mod onnx;
