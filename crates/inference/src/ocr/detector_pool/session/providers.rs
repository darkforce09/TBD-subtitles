//! The execution providers a detector session registers, with every option set.
//!
//! **Role:** build the CUDA provider (memory pool, arena growth, TF32, NHWC, CUDA graph, the
//! search mode's convolution search) and, for the TensorRT engine, the TensorRT provider before
//! it (FP16, engine and timing caches, a one-shape optimisation profile, workspace limit).
//!
//! **Position:** used by `ort_detector.rs` when it opens a session; plain values, so the tests
//! inspect the options without a GPU.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** TensorRT is registered before CUDA, so the nodes TensorRT does not take run on
//! CUDA inside the same session; every provider fails the session when it cannot register,
//! never falling back to the next provider or the CPU; under TensorRT the CUDA graph is
//! TensorRT's own, since ONNX Runtime captures a graph only when one provider runs every node.

use std::path::PathBuf;

use job_model::onscreen::DetectorEngine;
use ort::ep::{self, ArenaExtendStrategy, ExecutionProviderDispatch, cuda::ConvAlgorithmSearch};

use super::{CudaTuning, SessionSpec};
use crate::ocr::detector_pool::SearchMode;

const MIB: usize = 1024 * 1024;

/// How the TensorRT provider of one session is configured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorRtPlan {
    /// The key folder holding the engine and timing caches.
    pub cache_dir: PathBuf,
    /// `input:BxCxHxW`, the profile's minimum, optimum and maximum.
    pub profile: String,
    pub fp16: bool,
    pub workspace_mib: usize,
}

/// Whether the session asks ONNX Runtime for deterministic kernels.
pub fn deterministic_compute(search: SearchMode) -> bool {
    search == SearchMode::Deterministic
}

/// The CUDA provider for `spec`, with `tuning`'s CUDA graph unless TensorRT runs the graph.
pub fn cuda(spec: &SessionSpec, tuning: CudaTuning) -> ep::CUDA {
    let (search, max_workspace) = match spec.search {
        SearchMode::Fast => (ConvAlgorithmSearch::Exhaustive, true),
        SearchMode::Deterministic => (ConvAlgorithmSearch::Heuristic, false),
    };
    ep::CUDA::default()
        .with_device_id(0)
        .with_memory_limit(spec.pool_mib * MIB)
        .with_arena_extend_strategy(ArenaExtendStrategy::SameAsRequested)
        .with_conv_algorithm_search(search)
        .with_conv_max_workspace(max_workspace)
        .with_tf32(true)
        .with_prefer_nhwc(tuning.prefer_nhwc)
        .with_cuda_graph(tuning.cuda_graph && spec.engine == DetectorEngine::Cuda)
}

/// The TensorRT provider `plan` describes, with `tuning`'s CUDA graph.
pub fn tensorrt(plan: &TensorRtPlan, tuning: CudaTuning) -> ep::TensorRT {
    let cache = plan.cache_dir.display().to_string();
    ep::TensorRT::default()
        .with_device_id(0)
        .with_fp16(plan.fp16)
        .with_engine_cache(true)
        .with_engine_cache_path(&cache)
        .with_timing_cache(true)
        .with_timing_cache_path(&cache)
        .with_profile_min_shapes(&plan.profile)
        .with_profile_opt_shapes(&plan.profile)
        .with_profile_max_shapes(&plan.profile)
        .with_max_workspace_size(plan.workspace_mib * MIB)
        .with_cuda_graph(tuning.cuda_graph)
}

/// The providers in registration order: TensorRT first when `plan` is given, then CUDA; each
/// fails the session if it cannot register.
pub fn providers(
    spec: &SessionSpec,
    tuning: CudaTuning,
    plan: Option<&TensorRtPlan>,
) -> Vec<ExecutionProviderDispatch> {
    let mut list = Vec::with_capacity(2);
    if let Some(plan) = plan {
        list.push(tensorrt(plan, tuning).build().error_on_failure());
    }
    list.push(cuda(spec, tuning).build().error_on_failure());
    list
}

#[cfg(test)]
#[path = "tests/providers.rs"]
mod tests;
