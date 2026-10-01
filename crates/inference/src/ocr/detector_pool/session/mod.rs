//! One detector session: what opens it, what it runs, and its warm-up.
//!
//! **Role:** describe a session (model, fixed input shape, memory pool, engine and search mode),
//! open it through a `SessionOpener`, warm it up with black batches and, when it refuses the
//! CUDA graph or the NHWC layout, reopen it once without them and say so.
//!
//! **Position:** used by the pool's session threads; `OrtOpener` opens real ONNX Runtime
//! sessions, the tests open fakes through the same trait.
//!
//! **Signals and state:** a session owns its staging buffer and its output buffer; opening
//! returns the seconds spent warming up or building a TensorRT engine and the notes to report.
//!
//! **Invariants:** a session's input shape never changes after it opens; a refused option is
//! never dropped silently, the notes name it with the error; a session that fails again without
//! the options is an error, never a fall back to another provider.

mod engine_cache;
mod onnx_input;
mod ort_detector;
mod providers;

use std::path::PathBuf;
use std::time::Instant;

use job_model::onscreen::DetectorEngine;

use super::SearchMode;
use super::batch::{InputShape, fill_black};
use crate::ocr::OcrError;
use crate::ocr::pool::EngineIdentity;

pub use engine_cache::{TENSORRT_FOLDER, check_identity, workspace_mib};
pub(crate) use ort_detector::OrtOpener;

/// Black batches run after opening: the first runs the convolution search or builds the
/// engine, the second captures the CUDA graph.
pub const WARMUP_RUNS: usize = 2;

/// What a session does in the pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The mobile detector screening batches of frames.
    Screen,
    /// The server detector confirming single frames.
    Confirm,
}

impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Role::Screen => "screen",
            Role::Confirm => "confirm",
        }
    }
}

/// How the TensorRT provider builds and caches its engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorRtSpec {
    pub identity: EngineIdentity,
    /// The folder each cache key's folder goes in.
    pub cache_root: PathBuf,
    pub fp16: bool,
    /// The builder's and the engine's scratch memory limit, in MiB.
    pub workspace_mib: usize,
}

/// Everything a session is opened with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSpec {
    pub role: Role,
    pub model: PathBuf,
    pub input: InputShape,
    /// The CUDA provider's memory arena limit, in MiB.
    pub pool_mib: usize,
    pub engine: DetectorEngine,
    pub search: SearchMode,
    /// Present exactly when `engine` is TensorRT.
    pub tensorrt: Option<TensorRtSpec>,
}

/// The CUDA options a session may refuse and reopen without.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CudaTuning {
    pub cuda_graph: bool,
    pub prefer_nhwc: bool,
}

impl CudaTuning {
    /// Both options on: the first attempt.
    pub const FULL: CudaTuning = CudaTuning {
        cuda_graph: true,
        prefer_nhwc: true,
    };
    /// Both options off: the one reopen after a refusal.
    pub const PLAIN: CudaTuning = CudaTuning {
        cuda_graph: false,
        prefer_nhwc: false,
    };
}

/// A detector session with a fixed input shape.
pub trait DetectorSession {
    /// The input buffer the next run reads, `[batch, 3, height, width]`, written in place.
    fn staging(&mut self) -> &mut [f32];

    /// Run the staged batch and hand `read` the probability maps, `[batch, 1, height, width]`,
    /// where the run left them.
    fn run(&mut self, read: &mut dyn FnMut(&[f32]) -> Result<(), OcrError>)
    -> Result<(), OcrError>;
}

/// A session just opened.
pub struct Opened {
    pub session: Box<dyn DetectorSession>,
    /// The session had to build its TensorRT engine: no cached engine matched its key.
    pub builds_engine: bool,
    pub notes: Vec<(String, String)>,
}

/// Opens detector sessions; shared by the pool's threads, each opening its own.
pub trait SessionOpener: Send + Sync {
    fn open(&self, spec: &SessionSpec, tuning: CudaTuning) -> Result<Opened, OcrError>;
}

/// A session opened and warmed up.
pub struct WarmSession {
    pub session: Box<dyn DetectorSession>,
    pub warmup_s: f64,
    /// Seconds spent opening and warming up while the TensorRT engine was built; zero when a
    /// cached engine was used.
    pub engine_build_s: f64,
    pub notes: Vec<(String, String)>,
}

/// Open the session `spec` names with every CUDA option, warm it up, and when opening or
/// warming up fails, reopen it once without the CUDA graph and NHWC.
pub fn open_warm(opener: &dyn SessionOpener, spec: &SessionSpec) -> Result<WarmSession, OcrError> {
    let key = format!("{} session options", spec.role.label());
    match attempt(opener, spec, CudaTuning::FULL) {
        Ok(mut warm) => {
            warm.notes.push((key, "CUDA graph and NHWC on".to_owned()));
            Ok(warm)
        }
        Err(refusal) => {
            let mut warm = attempt(opener, spec, CudaTuning::PLAIN).map_err(|error| {
                format!(
                    "the {} detector failed with the CUDA graph and NHWC ({refusal}) and \
                     without them ({error})",
                    spec.role.label()
                )
            })?;
            warm.notes.push((
                key,
                format!("CUDA graph and NHWC off; the session refused them: {refusal}"),
            ));
            Ok(warm)
        }
    }
}

fn attempt(
    opener: &dyn SessionOpener,
    spec: &SessionSpec,
    tuning: CudaTuning,
) -> Result<WarmSession, OcrError> {
    let started = Instant::now();
    let Opened {
        mut session,
        builds_engine,
        notes,
    } = opener.open(spec, tuning)?;
    let opened_s = started.elapsed().as_secs_f64();
    let warming = Instant::now();
    fill_black(spec.input, session.staging())?;
    for _ in 0..WARMUP_RUNS {
        session.run(&mut |_| Ok(()))?;
    }
    let warmup_s = warming.elapsed().as_secs_f64();
    let (warmup_s, engine_build_s) = if builds_engine {
        (0.0, opened_s + warmup_s)
    } else {
        (warmup_s, 0.0)
    };
    Ok(WarmSession {
        session,
        warmup_s,
        engine_build_s,
        notes,
    })
}

#[cfg(test)]
#[path = "tests/session.rs"]
mod tests;
