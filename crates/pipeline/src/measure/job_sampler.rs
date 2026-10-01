//! The job's whole process tree and the GPU, sampled four times a second while a job runs.
//!
//! **Role:** read, every 250 ms, the device's SM, NVENC and NVDEC use through NVML, the summed
//! proportional set size of the app and every process under it, and their CPU ticks per process
//! and per thread; fold each sample into the windows of the steps running then.
//!
//! **Position:** started by the runner before it walks the steps, with this process's pid; the
//! runner opens a step's window just before the step runs and closes it right after, then merges
//! the step's `StepUse` into its measure. Uses `process_tree` for `/proc` and `step_use` for the
//! folding.
//!
//! **Signals and state:** one sampling thread, stopped through a channel so a stop never waits
//! out a sleep; the open windows behind a mutex shared with the runner's threads.
//!
//! **Invariants:** NVML exists on the host and not in the development container; without it the
//! GPU fields are `None`, never zero, and the memory and CPU fields are still measured; GPU use
//! is the whole device's, the desktop's share included; a step's CPU and memory are the whole
//! job's while it ran, so the shot scan and the visual lane running alongside count in the other
//! step's window too;
//! dropping the sampler stops its thread.

use std::collections::BTreeMap;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use job_model::StepName;
use nvml_wrapper::{Device, Nvml};

use super::process_tree;
use super::step_use::{Sample, StepUse, Windows, cpu_use};

const SAMPLE_EVERY: Duration = Duration::from_millis(250);

/// What the job used over the whole run.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct JobUse {
    /// The peak summed proportional set size of the job's processes, in MiB.
    pub peak_pss_mib: Option<f64>,
    /// How many samples were taken.
    pub samples: u64,
}

/// A thread sampling one process tree and the first GPU.
pub struct JobSampler {
    windows: Arc<Mutex<Windows>>,
    stop: Option<Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl JobSampler {
    /// Start sampling `root` and every process under it.
    pub fn start(root: u32) -> JobSampler {
        let windows = Arc::new(Mutex::new(Windows::default()));
        let (stop, stopped) = mpsc::channel::<()>();
        let shared = windows.clone();
        let thread = std::thread::Builder::new()
            .name("job-sampler".into())
            .spawn(move || {
                let nvml = Nvml::init().ok();
                let device = nvml.as_ref().and_then(|nvml| nvml.device_by_index(0).ok());
                let mut reader = Reader::new(root);
                loop {
                    let sample = reader.sample(device.as_ref());
                    lock(&shared).add(&sample);
                    match stopped.recv_timeout(SAMPLE_EVERY) {
                        Err(RecvTimeoutError::Timeout) => continue,
                        _ => break,
                    }
                }
            })
            .ok();
        JobSampler {
            windows,
            stop: Some(stop),
            thread,
        }
    }

    /// Start `step`'s window.
    pub fn open(&self, step: StepName) {
        lock(&self.windows).open(step);
    }

    /// End `step`'s window; what the job used while it was open.
    pub fn close(&self, step: StepName) -> StepUse {
        lock(&self.windows).close(step)
    }

    /// Stop sampling; what the job used over the run.
    pub fn stop(mut self) -> JobUse {
        self.halt();
        let windows = lock(&self.windows);
        JobUse {
            peak_pss_mib: windows.job().usage().job_ram_mib,
            samples: windows.job().samples(),
        }
    }

    fn halt(&mut self) {
        drop(self.stop.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for JobSampler {
    fn drop(&mut self) {
        self.halt();
    }
}

fn lock(windows: &Mutex<Windows>) -> MutexGuard<'_, Windows> {
    windows.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The previous reading of the tree's ticks, which the next sample's CPU use is measured from.
struct Reader {
    root: u32,
    clock_ticks: f64,
    previous: Option<Ticks>,
}

struct Ticks {
    at: Instant,
    processes: BTreeMap<u32, u64>,
    threads: BTreeMap<u32, u64>,
}

impl Reader {
    fn new(root: u32) -> Self {
        Self {
            root,
            clock_ticks: process_tree::clock_ticks(),
            previous: None,
        }
    }

    /// One reading of the tree and the device.
    fn sample(&mut self, device: Option<&Device<'_>>) -> Sample {
        let stats = process_tree::tree_stats(self.root);
        let pids: Vec<u32> = stats.keys().copied().collect();
        let now = Ticks {
            at: Instant::now(),
            processes: stats
                .iter()
                .map(|(&pid, stat)| (pid, stat.ticks()))
                .collect(),
            threads: process_tree::thread_ticks(pids.iter().copied()),
        };
        let cpu = self.previous.as_ref().and_then(|before| {
            cpu_use(
                (&before.processes, &before.threads),
                (&now.processes, &now.threads),
                now.at.duration_since(before.at).as_secs_f64(),
                self.clock_ticks,
            )
        });
        self.previous = Some(now);
        Sample {
            gpu_pct: device
                .and_then(|d| d.utilization_rates().ok())
                .map(|u| f64::from(u.gpu)),
            encoder_pct: device
                .and_then(|d| d.encoder_utilization().ok())
                .map(|u| f64::from(u.utilization)),
            decoder_pct: device
                .and_then(|d| d.decoder_utilization().ok())
                .map(|u| f64::from(u.utilization)),
            pss_mib: process_tree::pss_mib(pids),
            cpu_cores: cpu.map(|(cores, _)| cores),
            hot_thread_pct: cpu.map(|(_, hot)| hot),
        }
    }
}

#[cfg(test)]
#[path = "tests/job_sampler.rs"]
mod tests;
