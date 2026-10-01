//! One detector-pool configuration measured on the GPU.
//!
//! **Role:** open the production pool with one configuration, screen every held frame through
//! it as numbered jobs, or confirm stills through it, and measure the timed part's wall time and
//! GPU use, peak VRAM, the warm-up and engine build seconds the pool reports, its notes and the
//! regions found in frame order.
//! **Position:** called by `sections.rs` once per row; samples with `usage.rs` and the
//! pipeline's `gpu_monitor`.
//! **Signals and state:** one pool per call, dropped (its sessions closed) before returning.
//! **Invariants:** jobs and stills are copied before the clock starts and the pool is warm
//! before it starts; screening VRAM covers opening and screening, confirmation VRAM the
//! confirming after every confirm session opened; a pool error or panic becomes the row's error.

use std::panic::{AssertUnwindSafe, catch_unwind};

use inference::ocr::DetectorPool;
use inference::ocr::pool::{ConfirmJob, PaddedFrame, ScreenJob, TextScreening};
use pipeline::measure::gpu_monitor::Monitor;

use super::super::usage::Sampler;
use super::compare::Regions;
use super::plan::PoolRun;
use super::rows::Measured;
use super::{Setup, jobs};

/// Screen every frame of `frames` with `run`'s pool.
pub fn screen(setup: &Setup, frames: &[PaddedFrame], run: &PoolRun) -> Result<Measured, String> {
    let jobs = jobs(frames, run.shape.batch);
    let monitor = Monitor::start(std::process::id(), setup.baseline_mib);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        screen_jobs(setup, run, jobs, frames.len())
    }))
    .unwrap_or_else(|_| Err("the detector pool panicked".into()));
    let vram = monitor.finish();
    outcome.map(|measured| Measured { vram, ..measured })
}

fn screen_jobs(
    setup: &Setup,
    run: &PoolRun,
    jobs: Vec<ScreenJob>,
    frames: usize,
) -> Result<Measured, String> {
    let mut pool = DetectorPool::open(&setup.models_root, setup.options(run)).map_err(text)?;
    let count = jobs.len();
    let mut results = Vec::with_capacity(count);
    let sampler = Sampler::start();
    let screened = (|| {
        for job in jobs {
            pool.submit(job)?;
        }
        for _ in 0..count {
            results.push(pool.recv()?);
        }
        Ok::<_, inference::ocr::OcrError>(())
    })();
    let usage = sampler.finish();
    screened.map_err(text)?;
    results.sort_by_key(|result| result.seq);
    let regions: Regions = results.into_iter().flat_map(|r| r.regions).collect();
    if regions.len() != frames {
        return Err(format!(
            "{} region lists for {frames} frames",
            regions.len()
        ));
    }
    Ok(Measured {
        frames,
        usage,
        vram: None,
        warmup_s: pool.warmup_s(),
        engine_build_s: pool.engine_build_s(),
        notes: pool.notes(),
        regions,
    })
}

/// Confirm `stills` with `run`'s pool: one still per session first, untimed, so every confirm
/// session opens (and builds its engine) before the clock starts; then the rest, timed.
pub fn confirm(setup: &Setup, stills: &[PaddedFrame], run: &PoolRun) -> Result<Measured, String> {
    catch_unwind(AssertUnwindSafe(|| confirm_stills(setup, stills, run)))
        .unwrap_or_else(|_| Err("the detector pool panicked".into()))
}

fn confirm_stills(
    setup: &Setup,
    stills: &[PaddedFrame],
    run: &PoolRun,
) -> Result<Measured, String> {
    let warm = run.sessions.max(1);
    if stills.len() <= warm {
        return Err(format!(
            "{} stills leave none to time after {warm} warm ones",
            stills.len()
        ));
    }
    let confirm_jobs = |from: usize, frames: &[PaddedFrame]| -> Vec<ConfirmJob> {
        let numbered = frames.iter().enumerate();
        let jobs = numbered.map(|(offset, frame)| ConfirmJob {
            seq: (from + offset) as u64,
            frame: frame.clone(),
        });
        jobs.collect()
    };
    let (first, rest) = (
        confirm_jobs(0, &stills[..warm]),
        confirm_jobs(warm, &stills[warm..]),
    );
    let mut pool = DetectorPool::open(&setup.models_root, setup.options(run)).map_err(text)?;
    let (screen_warmup_s, screen_build_s) = (pool.warmup_s(), pool.engine_build_s());
    let warmed = pool.confirm(first).map_err(text)?;
    let monitor = Monitor::start(std::process::id(), setup.baseline_mib);
    let sampler = Sampler::start();
    let confirmed = pool.confirm(rest);
    let usage = sampler.finish();
    let vram = monitor.finish();
    let confirmed = confirmed.map_err(text)?;
    let regions: Regions = warmed
        .into_iter()
        .chain(confirmed)
        .map(|result| result.regions)
        .collect();
    Ok(Measured {
        frames: stills.len() - warm,
        usage,
        vram,
        warmup_s: pool.warmup_s() - screen_warmup_s,
        engine_build_s: pool.engine_build_s() - screen_build_s,
        notes: pool.notes(),
        regions,
    })
}

fn text(error: inference::ocr::OcrError) -> String {
    error.to_string()
}
