//! The detector pool: PP-OCRv5 sessions on our own ONNX Runtime sessions, side by side.
//!
//! **Role:** implement `TextScreening` with one thread per session. Each thread owns a mobile
//! detector session that screens padded full-resolution batches of a fixed shape; once screening
//! ends, the threads close those sessions and open server detector sessions, batch 1, that
//! confirm single frames. Sessions run on the CUDA provider, or on TensorRT before CUDA.
//!
//! **Position:** opened by the `text_detect` step in its GPU worker; the detection scan in
//! `stages` submits jobs and confirms through the `TextScreening` trait.
//!
//! **Signals and state:** the shared queue (`queue.rs`), one thread per session (`worker.rs`),
//! two result channels and the report of warm-up and engine-build times and notes.
//!
//! **Invariants:** probes run before waiting screening jobs; a job short of the batch is padded
//! with black frames, so the input shape never changes; screening results come back with their
//! job's number in any order, confirmations in the order given; screening and confirming
//! sessions never hold GPU memory at the same time; an unavailable provider, a session that
//! cannot open even without the CUDA graph and NHWC, and a TensorRT engine that cannot be built
//! are errors.

mod batch;
mod in_order;
mod queue;
mod regions;
mod session;
mod worker;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

use job_model::onscreen::DetectorEngine;

use super::OcrError;
use super::pool::{
    CONFIRM_POOL_MIB, ConfirmJob, ConfirmResult, EngineIdentity, SCREEN_SESSIONS, ScreenJob,
    ScreenResult, ScreenShape, TextScreening,
};
use batch::InputShape;
use queue::Queue;
use session::{Role, SessionOpener, SessionSpec, TensorRtSpec};
use worker::{Context, Report};

pub use in_order::InOrder;
pub use session::TENSORRT_FOLDER;

/// The mobile detector that screens.
const SCREEN_MODEL: &str = "pp-ocrv5/det_mobile.onnx";
/// The server detector that confirms.
const CONFIRM_MODEL: &str = "pp-ocrv5/det.onnx";
/// The GPU worker's memory cap, in MiB, that TensorRT's workspace is sized within unless the
/// caller gives another.
pub const DEFAULT_VRAM_CAP_MIB: usize = 6_656;

/// How cuDNN picks convolution algorithms.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SearchMode {
    /// Exhaustive search with the largest workspace: the fastest kernels, not reproducible.
    #[default]
    Fast,
    /// Heuristic search with ONNX Runtime's deterministic kernels: the same boxes every run.
    Deterministic,
}

impl SearchMode {
    pub fn label(self) -> &'static str {
        match self {
            SearchMode::Fast => "fast (exhaustive cuDNN search, largest workspace)",
            SearchMode::Deterministic => {
                "deterministic (heuristic cuDNN search, deterministic kernels)"
            }
        }
    }
}

/// How the pool opens its sessions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolOptions {
    pub engine: DetectorEngine,
    pub search: SearchMode,
    /// The card, driver and TensorRT build; read only by the TensorRT engine.
    pub identity: EngineIdentity,
    pub shape: ScreenShape,
    /// Screening sessions, one thread each.
    pub sessions: usize,
    /// The folder holding one folder per TensorRT engine key.
    pub cache_dir: PathBuf,
    /// The frames' own size; every frame of the run has it.
    pub frame_width: u32,
    pub frame_height: u32,
    /// Whether TensorRT builds FP16 engines.
    pub tensorrt_fp16: bool,
    /// The worker's GPU memory cap, in MiB, that TensorRT's workspace is sized within.
    pub vram_cap_mib: usize,
}

impl PoolOptions {
    /// The production options for frames of `width` × `height` on `engine`: the initial shape,
    /// two sessions, the fast search, FP16 engines cached under the app's data folder.
    pub fn new(
        engine: DetectorEngine,
        identity: EngineIdentity,
        width: u32,
        height: u32,
    ) -> Result<PoolOptions, OcrError> {
        let data = crate::model_store::app_data_dir().map_err(|error| error.to_string())?;
        Ok(PoolOptions {
            engine,
            search: SearchMode::default(),
            identity,
            shape: ScreenShape::INITIAL,
            sessions: SCREEN_SESSIONS,
            cache_dir: data.join(TENSORRT_FOLDER),
            frame_width: width,
            frame_height: height,
            tensorrt_fp16: true,
            vram_cap_mib: DEFAULT_VRAM_CAP_MIB,
        })
    }

    /// The session `role` opens with, its model under `models_root`.
    fn spec(&self, role: Role, models_root: &Path) -> SessionSpec {
        let (model, batch, pool_mib) = match role {
            Role::Screen => (SCREEN_MODEL, self.shape.batch, self.shape.pool_mib),
            Role::Confirm => (CONFIRM_MODEL, 1, CONFIRM_POOL_MIB),
        };
        let tensorrt = (self.engine == DetectorEngine::TensorRt).then(|| TensorRtSpec {
            identity: self.identity.clone(),
            cache_root: self.cache_dir.clone(),
            fp16: self.tensorrt_fp16,
            workspace_mib: session::workspace_mib(self.vram_cap_mib, self.sessions, pool_mib),
        });
        SessionSpec {
            role,
            model: models_root.join(model),
            input: InputShape::for_frames(batch, self.frame_width, self.frame_height),
            pool_mib,
            engine: self.engine,
            search: self.search,
            tensorrt,
        }
    }

    fn check(&self) -> Result<(), OcrError> {
        if self.sessions == 0 || self.shape.batch == 0 || self.shape.pool_mib == 0 {
            return Err("the detector pool needs a session, a batch and a memory pool".into());
        }
        if self.frame_width == 0 || self.frame_height == 0 {
            return Err("the detector pool needs the frames' size".into());
        }
        if self.engine == DetectorEngine::TensorRt {
            session::check_identity(&self.identity)?;
        }
        Ok(())
    }

    /// The notes that hold for the whole run.
    fn notes(&self, screen: &SessionSpec, confirm: &SessionSpec) -> BTreeMap<String, String> {
        let engine = match (self.engine, self.tensorrt_fp16) {
            (DetectorEngine::Cuda, _) => "CUDA".to_owned(),
            (DetectorEngine::TensorRt, fp16) => format!(
                "TensorRT ({}), then CUDA for the nodes TensorRT does not take; ONNX Runtime does \
                 not report that split",
                if fp16 { "FP16" } else { "FP32" }
            ),
        };
        let shape = |spec: &SessionSpec| {
            let [b, c, h, w] = spec.input.dims();
            format!("{b} × {c} × {h} × {w}, pool {} MiB", spec.pool_mib)
        };
        BTreeMap::from([
            ("detector engine".to_owned(), engine),
            ("search mode".to_owned(), self.search.label().to_owned()),
            ("screen sessions".to_owned(), self.sessions.to_string()),
            ("screen shape".to_owned(), shape(screen)),
            ("confirm shape".to_owned(), shape(confirm)),
        ])
    }
}

/// Detector sessions on their own threads, implementing `TextScreening`.
pub struct DetectorPool {
    queue: Arc<Queue>,
    screened: Receiver<Result<ScreenResult, OcrError>>,
    confirmed: Receiver<(usize, Result<ConfirmResult, OcrError>)>,
    threads: Vec<JoinHandle<()>>,
    report: Arc<Mutex<Report>>,
    notes: BTreeMap<String, String>,
    shape: ScreenShape,
    sessions: usize,
    screen_input: InputShape,
    frame: (u32, u32),
    outstanding: usize,
    screening_ended: bool,
}

impl DetectorPool {
    /// Open `options.sessions` screening sessions on the models under `models_root`, each warmed
    /// up before this returns.
    pub fn open(models_root: &Path, options: PoolOptions) -> Result<DetectorPool, OcrError> {
        super::strict_cuda_environment()?;
        DetectorPool::open_with(Arc::new(session::OrtOpener), models_root, options)
    }

    /// Open the pool with `opener` making its sessions.
    fn open_with(
        opener: Arc<dyn SessionOpener>,
        models_root: &Path,
        options: PoolOptions,
    ) -> Result<DetectorPool, OcrError> {
        options.check()?;
        let screen = options.spec(Role::Screen, models_root);
        let confirm = options.spec(Role::Confirm, models_root);
        let queue = Arc::new(Queue::new(options.sessions));
        let report = Arc::new(Mutex::new(Report::default()));
        let open_lock = Arc::new(Mutex::new(()));
        let (screened_tx, screened) = channel();
        let (confirmed_tx, confirmed) = channel();
        let (started_tx, started) = channel();
        let frame = (options.frame_width, options.frame_height);
        let mut threads = Vec::with_capacity(options.sessions);
        for thread in 0..options.sessions {
            let context = Context {
                queue: Arc::clone(&queue),
                opener: Arc::clone(&opener),
                open_lock: Arc::clone(&open_lock),
                report: Arc::clone(&report),
                screen: screen.clone(),
                confirm: confirm.clone(),
                frame,
                screened: screened_tx.clone(),
                confirmed: confirmed_tx.clone(),
            };
            let started = started_tx.clone();
            let spawned = std::thread::Builder::new()
                .name(format!("text-detector-{}", thread + 1))
                .spawn(move || worker::run(context, thread, started));
            match spawned {
                Ok(handle) => threads.push(handle),
                Err(error) => {
                    queue.update(|state| state.close());
                    threads.into_iter().for_each(|handle| drop(handle.join()));
                    return Err(format!("starting a detector thread: {error}").into());
                }
            }
        }
        drop(started_tx);
        let mut failure = None;
        for _ in 0..options.sessions {
            match started.recv() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => failure = failure.or(Some(error)),
                Err(_) => failure = failure.or(Some("a detector thread stopped".into())),
            }
        }
        let pool = DetectorPool {
            queue,
            screened,
            confirmed,
            threads,
            report,
            notes: options.notes(&screen, &confirm),
            shape: options.shape,
            sessions: options.sessions,
            screen_input: screen.input,
            frame,
            outstanding: 0,
            screening_ended: false,
        };
        match failure {
            Some(error) => Err(error),
            None => Ok(pool),
        }
    }

    fn report(&self) -> std::sync::MutexGuard<'_, Report> {
        self.report.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl TextScreening for DetectorPool {
    fn shape(&self) -> ScreenShape {
        self.shape
    }

    fn sessions(&self) -> usize {
        self.sessions
    }

    fn submit(&mut self, job: ScreenJob) -> Result<(), OcrError> {
        if self.screening_ended {
            return Err("screening has ended; the pool only confirms now".into());
        }
        batch::check_frames(&job.frames, self.screen_input, self.frame.0, self.frame.1)?;
        self.queue.update(|state| state.push_screen(job));
        self.outstanding += 1;
        Ok(())
    }

    fn recv(&mut self) -> Result<ScreenResult, OcrError> {
        if self.outstanding == 0 {
            return Err("no screening job is waiting for its result".into());
        }
        let result = self
            .screened
            .recv()
            .map_err(|_| "the detector threads stopped")?;
        self.outstanding -= 1;
        result
    }

    fn confirm(&mut self, jobs: Vec<ConfirmJob>) -> Result<Vec<ConfirmResult>, OcrError> {
        if self.outstanding > 0 {
            return Err(format!(
                "{} screening results are still due; screening must end before confirmation",
                self.outstanding
            )
            .into());
        }
        self.screening_ended = true;
        let count = jobs.len();
        self.queue
            .update(|state| state.push_confirm(jobs.into_iter().enumerate()));
        let mut answers: Vec<Option<ConfirmResult>> = vec![None; count];
        let mut failure = None;
        for _ in 0..count {
            let (index, result) = self
                .confirmed
                .recv()
                .map_err(|_| "the detector threads stopped")?;
            match result {
                Ok(answer) => answers[index] = Some(answer),
                Err(error) => failure = failure.or(Some(error)),
            }
        }
        if let Some(error) = failure {
            return Err(error);
        }
        answers
            .into_iter()
            .map(|answer| answer.ok_or_else(|| "a confirmation went unanswered".into()))
            .collect()
    }

    fn warmup_s(&self) -> f64 {
        self.report().warmup_s
    }

    fn engine_build_s(&self) -> f64 {
        self.report().engine_build_s
    }

    fn notes(&self) -> BTreeMap<String, String> {
        let mut notes = self.notes.clone();
        notes.extend(
            self.report()
                .notes
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );
        notes
    }
}

impl Drop for DetectorPool {
    fn drop(&mut self) {
        self.queue.update(|state| state.close());
        for handle in self.threads.drain(..) {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
#[path = "tests/pool.rs"]
mod tests;
