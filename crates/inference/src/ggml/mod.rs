//! Models run through ggml-based crates; each bundles its own ggml, so only one of them links
//! into a binary. crispasr is the one: it runs Whisper and the Qwen3 forced aligner on CUDA.

#[cfg(feature = "crispasr")]
pub mod crispasr;
