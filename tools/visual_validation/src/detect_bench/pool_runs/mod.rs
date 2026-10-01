//! The production detector pool under the benchmark: sweeps, sessions, search modes, TensorRT.
//!
//! **Role:** turn the clip's yuv420p samples into the padded rgb24 frames the pool takes, open
//! `inference::ocr::DetectorPool` with one configuration at a time and measure its screening
//! and confirmation, and compare the boxes each configuration finds with a CUDA reference run.
//! **Position:** called by `detect_bench::run` after the oar-ocr sections; `sections.rs` prints
//! the tables, `measure.rs` drives the pool, `plan.rs`, `rows.rs` and `compare.rs` are pure.
//! **Signals and state:** the padded frames in memory; one pool open at a time, closed before
//! the next opens.
//! **Invariants:** every configuration gives a row, a failure its error; frames are converted
//! once, before any timing, in the stream's own matrix and range.

mod compare;
mod measure;
mod plan;
mod rows;
mod sections;

use std::path::PathBuf;

use inference::ocr::PoolOptions;
use inference::ocr::pool::{CONFIRM_SESSIONS, EngineIdentity, PaddedFrame, Priority, ScreenJob};
use media_io::yuv::{self, Coefficients, Yuv420};
use pipeline::measure::gpu_monitor::DeviceInfo;

pub use compare::Regions;
pub use plan::{PoolRun, grid};
pub use sections::{Sections, run as run_sections};

/// What every pool configuration shares.
#[derive(Debug, Clone)]
pub struct Setup {
    /// The model store holding `pp-ocrv5/`.
    pub models_root: PathBuf,
    pub identity: EngineIdentity,
    /// The folder TensorRT engines are built into and reused from.
    pub cache_dir: PathBuf,
    pub vram_cap_mib: usize,
    /// The frames' own width and height.
    pub frame: (u32, u32),
    /// Device memory in use before the bench touched CUDA, in MiB.
    pub baseline_mib: u64,
}

impl Setup {
    /// The pool's options for `run`.
    pub fn options(&self, run: &PoolRun) -> PoolOptions {
        PoolOptions {
            engine: run.engine,
            search: run.search,
            identity: self.identity.clone(),
            shape: run.shape,
            sessions: run.sessions,
            confirm_sessions: CONFIRM_SESSIONS.min(run.sessions),
            confirm_pool_mib: run.confirm_pool_mib,
            cache_dir: self.cache_dir.clone(),
            frame_width: self.frame.0,
            frame_height: self.frame.1,
            tensorrt_fp16: run.fp16,
            vram_cap_mib: self.vram_cap_mib,
        }
    }
}

/// The card and driver NVML reports and the bundled TensorRT build; empty names without NVML,
/// which only the CUDA engine accepts.
pub fn identity(device: Option<&DeviceInfo>) -> EngineIdentity {
    EngineIdentity {
        gpu_name: device.map(|d| d.name.clone()).unwrap_or_default(),
        driver: device.map(|d| d.driver.clone()).unwrap_or_default(),
        tensorrt_version: inference::model_store::manifest::TENSORRT_VERSION.to_string(),
    }
}

/// One yuv420p picture of `size` converted to rgb24 and padded below with black rows.
pub fn padded_frame(
    bytes: &[u8],
    (width, height): (u32, u32),
    colour: &Coefficients,
) -> Result<PaddedFrame, String> {
    let picture = Yuv420::planar(bytes, width as usize, height as usize).ok_or_else(|| {
        format!(
            "{} bytes are no {width}×{height} yuv420p picture",
            bytes.len()
        )
    })?;
    let padded_height = PaddedFrame::padded(height);
    let mut rgb = vec![0u8; width as usize * padded_height as usize * 3];
    if !yuv::to_rgb_padded_into(&picture, colour, padded_height as usize, &mut rgb) {
        return Err("the padded buffer does not fit the picture".into());
    }
    Ok(PaddedFrame {
        width,
        height,
        padded_height,
        rgb,
    })
}

/// `frames` cut into screening jobs of `batch`, numbered from zero; the last may be short.
pub fn jobs(frames: &[PaddedFrame], batch: usize) -> Vec<ScreenJob> {
    frames
        .chunks(batch.max(1))
        .enumerate()
        .map(|(seq, chunk)| ScreenJob {
            seq: seq as u64,
            priority: Priority::Screen,
            frames: chunk.to_vec(),
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/setup.rs"]
mod tests;
