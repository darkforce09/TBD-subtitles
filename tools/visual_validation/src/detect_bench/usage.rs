//! What one measured run of the detection benchmark used: GPU, NVDEC and CPU.
//!
//! **Role:** sample the first GPU's SM and NVDEC utilisation through NVML every 100 ms while a
//! run goes, and count this process's CPU ticks, its own and its reaped children's, from start to
//! finish.
//! **Position:** wrapped around each timed part of `detect-bench`; peak VRAM comes from the
//! pipeline's own `gpu_monitor`.
//! **Signals and state:** one sampling thread per run, stopped by an atomic flag.
//! **Invariants:** without NVML the GPU figures are `None`, never zero; a child's ticks count only
//! once it is reaped, so a run waits for its FFmpeg before finishing.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use nvml_wrapper::Nvml;

const SAMPLE_EVERY: Duration = Duration::from_millis(100);

/// The means of one run.
#[derive(Debug, Clone, Copy)]
pub struct Usage {
    pub wall_s: f64,
    /// Busy CPU cores: this process's and its reaped children's ticks over the wall time.
    pub cpu_cores: Option<f64>,
    pub gpu_pct: Option<f64>,
    pub decoder_pct: Option<f64>,
}

/// Sums of the utilisation samples.
#[derive(Default)]
struct Sums {
    gpu: f64,
    decoder: f64,
    samples: u32,
}

/// A run being measured.
pub struct Sampler {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<Option<Sums>>,
    started: Instant,
    ticks: Option<u64>,
}

impl Sampler {
    /// Start the clock, the tick count and the sampling thread.
    pub fn start() -> Sampler {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::spawn(move || {
            let nvml = Nvml::init().ok()?;
            let device = nvml.device_by_index(0).ok()?;
            let mut sums = Sums::default();
            while !flag.load(Ordering::SeqCst) {
                std::thread::sleep(SAMPLE_EVERY);
                if let (Ok(rates), Ok(decoder)) =
                    (device.utilization_rates(), device.decoder_utilization())
                {
                    sums.gpu += f64::from(rates.gpu);
                    sums.decoder += f64::from(decoder.utilization);
                    sums.samples += 1;
                }
            }
            Some(sums)
        });
        Sampler {
            stop,
            thread,
            started: Instant::now(),
            ticks: own_ticks_now(),
        }
    }

    /// Stop sampling and return the run's means.
    pub fn finish(self) -> Usage {
        let wall_s = self.started.elapsed().as_secs_f64();
        let ticks = own_ticks_now();
        self.stop.store(true, Ordering::SeqCst);
        let sums = self.thread.join().ok().flatten();
        let mean = |sum: f64, sums: &Sums| sum / f64::from(sums.samples);
        let sums = sums.filter(|sums| sums.samples > 0);
        Usage {
            wall_s,
            cpu_cores: match (self.ticks, ticks) {
                (Some(before), Some(after)) if wall_s > 0.0 => Some(
                    after.saturating_sub(before) as f64
                        / pipeline::measure::process_tree::clock_ticks()
                        / wall_s,
                ),
                _ => None,
            },
            gpu_pct: sums.as_ref().map(|s| mean(s.gpu, s)),
            decoder_pct: sums.as_ref().map(|s| mean(s.decoder, s)),
        }
    }
}

/// This process's user and system ticks plus those of its reaped children.
fn own_ticks_now() -> Option<u64> {
    own_ticks(&std::fs::read_to_string("/proc/self/stat").ok()?)
}

/// Fields 14 to 17 of a `/proc/<pid>/stat` line summed: utime, stime, cutime and cstime.
pub fn own_ticks(stat: &str) -> Option<u64> {
    // The command name may hold spaces and parentheses; the fields after its last `)` start at 3.
    let rest = &stat[stat.rfind(')')? + 1..];
    let fields: Vec<&str> = rest.split_whitespace().collect();
    let mut total = 0u64;
    for field in fields.get(11..15)? {
        total = total.checked_add(field.parse::<i64>().ok()?.max(0) as u64)?;
    }
    Some(total)
}

#[cfg(test)]
#[path = "tests/usage.rs"]
mod tests;
