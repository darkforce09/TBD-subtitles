//! The screening pool's contract: what the detection scan hands the detector sessions and what
//! comes back, so the scan and the sessions are written and tested apart.
//!
//! **Role:** name a batch of padded full-resolution frames, its place in the queue, the regions
//! found on each frame, the confirmation of one frame by the server detector, and the shape and
//! memory the sessions run with.
//!
//! **Position:** implemented by the detector pool in this module's folder; used by the detection
//! scan in `stages`; scripted by the scan's tests.
//!
//! **Signals and state:** plain values and one trait.
//!
//! **Invariants:** every frame of a job has the same size; a result carries the sequence number
//! of its job and one region list per frame in the job's order; regions are in frame pixels,
//! clipped to the frame above its padding; results of screening jobs may come back in any order,
//! confirmations come back in the order asked.

use std::collections::BTreeMap;

use job_model::onscreen::Quad;

use super::OcrError;

/// How many frames one screening call takes and the GPU memory pool its session may hold: one
/// measured pair, since a larger batch needs a larger pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenShape {
    pub batch: usize,
    pub pool_mib: usize,
}

impl ScreenShape {
    /// The shape screening runs at, from the host's pool-by-batch sweep on Dressrosa 11 and 28:
    /// four frames per session, so two sessions keep eight in flight, in a 1,536 MiB pool, the
    /// fastest pair on CUDA (35 to 44 frames a second) and on TensorRT FP16 (239 to 242).
    pub const INITIAL: ScreenShape = ScreenShape {
        batch: 4,
        pool_mib: 1_536,
    };
}

/// The screening sessions that run side by side on the GPU.
pub const SCREEN_SESSIONS: usize = 2;

/// The GPU memory pool of each confirming session, in MiB: the smallest of the host's sweep in
/// which the server detector confirms a full-resolution still on CUDA (2,560 MiB runs out); the
/// screening sessions are closed before these open.
pub const CONFIRM_POOL_MIB: usize = 3_072;

/// The width every screened frame is also shrunk to, for writing too large for the mobile
/// detector at full resolution: a glyph a third of the frame tall is found at this width.
pub const PROXY_WIDTH: u32 = 640;

/// The GPU memory pool of each proxy screening session, in MiB.
pub const PROXY_POOL_MIB: usize = 512;

/// The proxy size of a `width` × `height` frame: `PROXY_WIDTH` wide, the height in proportion and
/// even, never larger than the frame.
pub fn proxy_size(width: u32, height: u32) -> (u32, u32) {
    if width <= PROXY_WIDTH {
        return (width, height);
    }
    let scaled = (u64::from(PROXY_WIDTH) * u64::from(height) / u64::from(width.max(1))) as u32;
    (PROXY_WIDTH, (scaled & !1).max(2))
}

/// The confirming sessions: one, since a single server detector on a full-resolution frame keeps
/// the GPU busy and a second does not fit beside it within the worker's VRAM cap.
pub const CONFIRM_SESSIONS: usize = 1;

/// One frame converted to rgb24 and padded below with black rows to a multiple of 32.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaddedFrame {
    pub width: u32,
    /// The picture's own height; rows from here down are padding.
    pub height: u32,
    pub padded_height: u32,
    /// `width` × `padded_height` × 3 bytes.
    pub rgb: Vec<u8>,
}

impl PaddedFrame {
    /// The height `height` pads to: the next multiple of 32.
    pub fn padded(height: u32) -> u32 {
        height.div_ceil(32) * 32
    }
}

/// Which work the pool runs first when both wait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Bisection probes: the scan waits on them before it can move on.
    Probe,
    /// Screening batches decoded ahead of the scan.
    Screen,
}

/// Frames to screen together, at most `ScreenShape::batch` of them.
#[derive(Debug, Clone)]
pub struct ScreenJob {
    pub seq: u64,
    pub priority: Priority,
    pub frames: Vec<PaddedFrame>,
}

/// The regions scoring at least the screening score on each frame of a job.
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenResult {
    pub seq: u64,
    pub regions: Vec<Vec<(Quad, f64)>>,
}

/// One frame for the server detector to confirm.
#[derive(Debug, Clone)]
pub struct ConfirmJob {
    pub seq: u64,
    pub frame: PaddedFrame,
}

/// The regions scoring at least the confirmation score on a confirmed frame.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfirmResult {
    pub seq: u64,
    pub regions: Vec<(Quad, f64)>,
}

/// The card, driver and TensorRT build an engine is made for; a cached engine is reused only
/// for the same identity. The caller reads it from the driver, since this crate does not.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EngineIdentity {
    pub gpu_name: String,
    pub driver: String,
    pub tensorrt_version: String,
}

/// Detector sessions that screen batches side by side and confirm single frames.
pub trait TextScreening: Send {
    /// The batch and pool the screening sessions run with.
    fn shape(&self) -> ScreenShape;

    /// How many screening sessions run side by side.
    fn sessions(&self) -> usize;

    /// Queue a screening job; jobs of `Priority::Probe` run before waiting screening jobs.
    fn submit(&mut self, job: ScreenJob) -> Result<(), OcrError>;

    /// The next finished screening job's regions, in whatever order jobs finish; blocks until
    /// one does.
    fn recv(&mut self) -> Result<ScreenResult, OcrError>;

    /// Confirm every frame with the server detector, spread over the sessions; one result per
    /// job, in the order given. Screening ends before the first confirmation.
    fn confirm(&mut self, jobs: Vec<ConfirmJob>) -> Result<Vec<ConfirmResult>, OcrError>;

    /// Seconds spent warming the sessions up when they opened, outside screening time.
    fn warmup_s(&self) -> f64;

    /// Seconds spent building TensorRT engines on this run; zero when cached engines were used.
    fn engine_build_s(&self) -> f64;

    /// What the sessions recorded about how they run: the engine, the options a session refused
    /// and reopened without, the providers that took each part of the graph.
    fn notes(&self) -> BTreeMap<String, String>;
}

#[cfg(test)]
#[path = "tests/pool.rs"]
mod tests;
