//! Which detector-pool configurations the benchmark runs.
//!
//! **Role:** name one pool configuration (engine, precision, search mode, sessions, batch and
//! memory pool), lay out the batch-by-pool sweep grid, and pick the shape the later sections
//! run at: the one the caller gives, else the fastest measured, else the production default.
//! **Position:** used by the pool sections of `detect-bench` and by their row formatting.
//! **Signals and state:** none; plain values.
//! **Invariants:** the grid holds every usable batch and pool pair once, batches in the order
//! given and each batch's pools in the order given, so a TensorRT engine (keyed by batch, not
//! pool) is built at the first pool size of its batch and reused at the others.

use inference::ocr::SearchMode;
use inference::ocr::pool::{CONFIRM_POOL_MIB, ScreenShape};
use job_model::onscreen::DetectorEngine;

/// One configuration of the production detector pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolRun {
    pub engine: DetectorEngine,
    /// Whether TensorRT builds an FP16 engine; read only for TensorRT.
    pub fp16: bool,
    pub search: SearchMode,
    pub sessions: usize,
    pub shape: ScreenShape,
    /// Each confirming session's GPU memory pool, in MiB.
    pub confirm_pool_mib: usize,
}

impl PoolRun {
    /// The CUDA provider with the fast search.
    pub fn cuda(shape: ScreenShape, sessions: usize) -> PoolRun {
        PoolRun {
            engine: DetectorEngine::Cuda,
            fp16: false,
            search: SearchMode::Fast,
            sessions,
            shape,
            confirm_pool_mib: CONFIRM_POOL_MIB,
        }
    }

    /// TensorRT before CUDA, at `fp16` or FP32 precision, with the fast search.
    pub fn tensorrt(shape: ScreenShape, sessions: usize, fp16: bool) -> PoolRun {
        PoolRun {
            engine: DetectorEngine::TensorRt,
            fp16,
            ..PoolRun::cuda(shape, sessions)
        }
    }

    /// The engine and, for TensorRT, its precision.
    pub fn engine_label(&self) -> &'static str {
        match (self.engine, self.fp16) {
            (DetectorEngine::Cuda, _) => "CUDA",
            (DetectorEngine::TensorRt, true) => "TensorRT FP16",
            (DetectorEngine::TensorRt, false) => "TensorRT FP32",
        }
    }

    pub fn search_label(&self) -> &'static str {
        match self.search {
            SearchMode::Fast => "fast",
            SearchMode::Deterministic => "deterministic",
        }
    }
}

/// Every pair of a non-zero batch and a non-zero pool, each once.
pub fn grid(batches: &[usize], pools_mib: &[usize]) -> Vec<ScreenShape> {
    let mut shapes: Vec<ScreenShape> = Vec::new();
    for &batch in batches.iter().filter(|&&batch| batch > 0) {
        for &pool_mib in pools_mib.iter().filter(|&&pool| pool > 0) {
            let shape = ScreenShape { batch, pool_mib };
            if !shapes.contains(&shape) {
                shapes.push(shape);
            }
        }
    }
    shapes
}

/// The shape of the fastest measured row, the first of equals.
pub fn fastest(measured: &[(ScreenShape, f64)]) -> Option<ScreenShape> {
    measured
        .iter()
        .filter(|(_, fps)| fps.is_finite())
        .fold(
            None,
            |best: Option<(ScreenShape, f64)>, &(shape, fps)| match best {
                Some((_, top)) if top >= fps => best,
                _ => Some((shape, fps)),
            },
        )
        .map(|(shape, _)| shape)
}

/// The shape the later sections run at: `given`, else the fastest of `measured`, else the
/// production default.
pub fn chosen(given: Option<ScreenShape>, measured: &[(ScreenShape, f64)]) -> ScreenShape {
    given
        .or_else(|| fastest(measured))
        .unwrap_or(ScreenShape::INITIAL)
}

#[cfg(test)]
#[path = "tests/plan.rs"]
mod tests;
