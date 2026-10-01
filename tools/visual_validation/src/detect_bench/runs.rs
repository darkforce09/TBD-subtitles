//! The detector sections of the detection benchmark: screening and confirmation rows.
//!
//! **Role:** run one detector configuration (path, batch, workers, arena limit) over frames held
//! in memory, with every worker's session opened and warmed on one batch before the clock
//! starts, and measure ms per frame, frames per second, GPU use, peak VRAM, CPU cores and boxes
//! found; lay the configurations out as the screening and confirmation tables.
//! **Position:** called by `detect_bench::run` with the decoded sample frames; opens detectors
//! from `detector.rs` and samples with `usage.rs` and the pipeline's `gpu_monitor`.
//! **Signals and state:** one thread per worker, each owning its own detector; batches go to
//! workers round-robin and come back reordered.
//! **Invariants:** a failed configuration prints its error in its row and is tried once more at
//! the raised arena limit; the timed part excludes opening, warming and decoding.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::Barrier;

use image::RgbImage;
use pipeline::measure::gpu_monitor::{Monitor, VramPeaks};

use super::detector::{Padded, Screen, Screened, Settings, Stock};
use super::table::{Table, optional};
use super::usage::{Sampler, Usage};

/// Which detector path a row runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    Stock,
    Padded,
}

/// One measured configuration.
#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub way: Way,
    pub batch: usize,
    pub workers: usize,
    pub memory_limit_mib: u64,
    pub limit_side: Option<u32>,
    pub box_score: f32,
}

/// What one configuration measured.
pub struct Measured {
    pub frames: usize,
    pub usage: Usage,
    pub vram: Option<VramPeaks>,
    pub boxes: usize,
}

impl Measured {
    pub fn fps(&self) -> f64 {
        self.frames as f64 / self.usage.wall_s
    }
}

/// The arena limits and the device baseline every row shares.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub memory_limit_mib: u64,
    pub raised_limit_mib: u64,
    /// Device memory in use before the bench touched CUDA, in MiB.
    pub baseline_mib: u64,
}

/// Run `config` with `model` over `frames`, sampling peak VRAM across opening and screening.
pub fn measure(
    model: &Path,
    frames: &[RgbImage],
    config: &Config,
    baseline_mib: u64,
) -> Result<Measured, String> {
    let monitor = Monitor::start(std::process::id(), baseline_mib);
    let result = screen_all(model, frames, config);
    let vram = monitor.finish();
    let (screened, usage) = result?;
    let boxes = screened
        .iter()
        .map(|regions| {
            let kept = regions.iter().filter(|(_, s)| *s >= config.box_score);
            kept.count()
        })
        .sum();
    Ok(Measured {
        frames: screened.len(),
        usage,
        vram,
        boxes,
    })
}

/// Open one detector of `config`'s path.
fn open(config: &Config, model: &Path, frame: (u32, u32)) -> Result<Box<dyn Screen>, String> {
    let settings = Settings {
        model,
        limit_side: config.limit_side,
        box_score: config.box_score,
        memory_limit_mib: config.memory_limit_mib,
    };
    Ok(match config.way {
        Way::Stock => Box::new(Stock::open(&settings)?),
        Way::Padded => Box::new(Padded::open(&settings, frame)?),
    })
}

/// Every frame's regions in input order, and what the timed part used.
fn screen_all(
    model: &Path,
    frames: &[RgbImage],
    config: &Config,
) -> Result<(Screened, Usage), String> {
    let first = frames.first().ok_or("no frames to screen")?;
    let size = first.dimensions();
    let chunks: Vec<&[RgbImage]> = frames.chunks(config.batch.max(1)).collect();
    let workers = config.workers.max(1);
    let barrier = Barrier::new(workers + 1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|worker| {
                let (chunks, barrier) = (&chunks, &barrier);
                scope.spawn(move || {
                    let ready = catch_unwind(AssertUnwindSafe(|| {
                        let mut detector = open(config, model, size)?;
                        detector.screen(&chunks[0].iter().collect::<Vec<_>>())?;
                        Ok::<_, String>(detector)
                    }));
                    barrier.wait();
                    let mut detector = ready.map_err(|_| "opening a detector panicked")??;
                    let mut screened = Vec::new();
                    for (index, chunk) in chunks.iter().enumerate().skip(worker).step_by(workers) {
                        let refs: Vec<&RgbImage> = chunk.iter().collect();
                        screened.push((index, detector.screen(&refs)?));
                    }
                    Ok::<_, String>(screened)
                })
            })
            .collect();
        barrier.wait();
        let sampler = Sampler::start();
        let joined: Vec<_> = handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|_| Err("a worker panicked".to_string()))
            })
            .collect();
        let usage = sampler.finish();
        let mut batches = Vec::new();
        for result in joined {
            batches.extend(result?);
        }
        batches.sort_by_key(|(index, _)| *index);
        let screened: Screened = batches.into_iter().flat_map(|(_, s)| s).collect();
        if screened.len() != frames.len() {
            return Err(format!(
                "{} region lists for {} frames",
                screened.len(),
                frames.len()
            ));
        }
        Ok((screened, usage))
    })
}

/// The screening table's columns.
const SCREEN_COLUMNS: [&str; 13] = [
    "Path",
    "Batch",
    "Workers",
    "Limit MiB",
    "Frames",
    "ms/frame",
    "Frames/s",
    "GPU %",
    "VRAM process MiB",
    "VRAM device Δ MiB",
    "CPU cores",
    "Boxes",
    "Note",
];

/// The row describing `config` and its outcome, under `path`.
fn row(path: &str, config: &Config, outcome: &Result<Measured, String>) -> Vec<String> {
    let mut cells = vec![
        path.to_string(),
        config.batch.to_string(),
        config.workers.to_string(),
        config.memory_limit_mib.to_string(),
    ];
    match outcome {
        Ok(m) => cells.extend([
            m.frames.to_string(),
            format!("{:.2}", m.usage.wall_s * 1000.0 / m.frames as f64),
            format!("{:.1}", m.fps()),
            optional(m.usage.gpu_pct, 0),
            optional(m.vram.map(|v| v.process_mib as f64), 0),
            optional(m.vram.map(|v| v.device_delta_mib as f64), 0),
            optional(m.usage.cpu_cores, 1),
            m.boxes.to_string(),
            String::new(),
        ]),
        Err(error) => {
            cells.extend(std::iter::repeat_n(String::new(), 8));
            cells.push(format!("error: {error}"));
        }
    }
    cells
}

/// Measure `config`, add its row, and on failure add a second row at the raised limit; the
/// successful outcome and the limit it ran at.
fn measure_into(
    table: &mut Table,
    path: &str,
    model: &Path,
    frames: &[RgbImage],
    config: Config,
    limits: &Limits,
) -> Option<(Measured, Config)> {
    let outcome = measure(model, frames, &config, limits.baseline_mib);
    table.row(row(path, &config, &outcome));
    eprintln!("{path} batch {} ×{} done", config.batch, config.workers);
    match outcome {
        Ok(measured) => Some((measured, config)),
        Err(_) if limits.raised_limit_mib > config.memory_limit_mib => {
            let raised = Config {
                memory_limit_mib: limits.raised_limit_mib,
                ..config
            };
            let outcome = measure(model, frames, &raised, limits.baseline_mib);
            table.row(row(path, &raised, &outcome));
            outcome.ok().map(|measured| (measured, raised))
        }
        Err(_) => None,
    }
}

/// Section 2: the mobile detector at full resolution, stock and padded, then two workers at each
/// path's fastest batch, then the proxy baseline.
pub fn screening(model: &Path, samples: &[RgbImage], proxies: &[RgbImage], limits: &Limits) {
    let side = samples.first().map(|f| f.width().max(f.height()));
    let base = Config {
        way: Way::Stock,
        batch: 1,
        workers: 1,
        memory_limit_mib: limits.memory_limit_mib,
        limit_side: side,
        box_score: 0.3,
    };
    println!(
        "## 2. Mobile detector at full resolution ({} sample frames; boxes scoring at least 0.3)\n",
        samples.len()
    );
    let mut table = Table::new(&SCREEN_COLUMNS);
    let paths = [
        (
            Way::Stock,
            "stock, limit 1920 (stretched)",
            &[1usize, 2, 4, 8][..],
        ),
        (
            Way::Padded,
            "padded to 32, parallel post-process",
            &[2, 4, 8][..],
        ),
    ];
    for (way, label, batches) in paths {
        let mut best: Option<(f64, Config)> = None;
        for &batch in batches {
            let config = Config { way, batch, ..base };
            if let Some((m, used)) = measure_into(&mut table, label, model, samples, config, limits)
                && best.is_none_or(|(fps, _)| m.fps() > fps)
            {
                best = Some((m.fps(), used));
            }
        }
        if let Some((_, fastest)) = best {
            let config = Config {
                workers: 2,
                ..fastest
            };
            measure_into(&mut table, label, model, samples, config, limits);
        }
    }
    let proxy = Config {
        batch: 4,
        limit_side: None,
        ..base
    };
    let label = "proxy baseline: stock, 640 × 360";
    measure_into(&mut table, label, model, proxies, proxy, limits);
    println!("{}", table.render());
}

/// Section 3: the server detector on single full-resolution stills, at limit 1920 and at the
/// predictor's default.
pub fn confirmation(model: &Path, samples: &[RgbImage], limits: &Limits) {
    println!(
        "## 3. Server detector, batch 1 ({} stills; boxes scoring at least 0.5)\n",
        samples.len()
    );
    let mut table = Table::new(&SCREEN_COLUMNS);
    let side = samples.first().map(|f| f.width().max(f.height()));
    for (limit_side, label) in [
        (side, "det.onnx, limit 1920 (stretched)"),
        (None, "det.onnx, default limit 960"),
    ] {
        let config = Config {
            way: Way::Stock,
            batch: 1,
            workers: 1,
            memory_limit_mib: limits.memory_limit_mib,
            limit_side,
            box_score: 0.5,
        };
        measure_into(&mut table, label, model, samples, config, limits);
    }
    println!("{}", table.render());
}
