//! What the job's processes used while a step ran, folded from the sampler's samples: GPU, NVENC
//! and NVDEC use, whole-job memory, CPU cores and the busiest thread.
//!
//! **Role:** turn two readings of the job's CPU ticks into cores busy and the busiest thread's
//! share, and fold each sample into every window open at the time, so a step's use is the
//! aggregate of the samples taken while it ran.
//!
//! **Position:** fed by `job_sampler`'s thread; read by the runner through
//! `JobSampler::close`, which hands a step its `StepUse` to merge into its `StepMeasure`. Pure, so
//! the tests feed samples directly.
//!
//! **Signals and state:** one accumulator per open window and one for the whole job; each keeps
//! sums, counts and peaks, never the samples themselves, so memory stays bounded.
//!
//! **Invariants:** a value no sample measured stays `None`, never zero; overlapping windows (the
//! shot scan or a visual lane step beside another step) each take every sample; a sample's CPU use is that of the
//! interval since the previous sample, a pid or thread new since then counts its ticks in full,
//! and one gone since then loses the ticks of its last interval.

use std::collections::BTreeMap;

use job_model::StepName;
use job_model::job::StepMeasure;

/// One reading of the job and the device; `None` where it could not be measured.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Sample {
    /// The device's SM use, in percent.
    pub gpu_pct: Option<f64>,
    /// The device's NVENC use, in percent.
    pub encoder_pct: Option<f64>,
    /// The device's NVDEC use, in percent.
    pub decoder_pct: Option<f64>,
    /// The summed proportional set size of the job's processes, in MiB.
    pub pss_mib: Option<f64>,
    /// The CPU cores the job's processes kept busy since the previous sample.
    pub cpu_cores: Option<f64>,
    /// The share of one core the busiest thread used since the previous sample, in percent.
    pub hot_thread_pct: Option<f64>,
}

/// The CPU use between two readings of the job's ticks, by pid and by thread id: the cores busy
/// and the busiest thread's share of one core in percent. `None` when no time passed.
pub fn cpu_use(
    previous: (&BTreeMap<u32, u64>, &BTreeMap<u32, u64>),
    current: (&BTreeMap<u32, u64>, &BTreeMap<u32, u64>),
    elapsed_s: f64,
    clock_ticks: f64,
) -> Option<(f64, f64)> {
    if elapsed_s <= 0.0 || clock_ticks <= 0.0 {
        return None;
    }
    let busy = |ticks: u64| ticks as f64 / clock_ticks / elapsed_s;
    let total: u64 = deltas(previous.0, current.0).sum();
    let hottest = deltas(previous.1, current.1).max().unwrap_or(0);
    Some((busy(total), busy(hottest) * 100.0))
}

/// Each id's ticks since `previous`: in full for an id new since then; a reused id whose count
/// fell counts none.
fn deltas<'a>(
    previous: &'a BTreeMap<u32, u64>,
    current: &'a BTreeMap<u32, u64>,
) -> impl Iterator<Item = u64> + 'a {
    current
        .iter()
        .map(|(id, &now)| now.saturating_sub(previous.get(id).copied().unwrap_or(0)))
}

/// A running mean.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Mean {
    sum: f64,
    count: u64,
}

impl Mean {
    fn add(&mut self, value: Option<f64>) {
        if let Some(value) = value {
            self.sum += value;
            self.count += 1;
        }
    }

    fn value(&self) -> Option<f64> {
        (self.count > 0).then(|| self.sum / self.count as f64)
    }
}

fn peak(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, None) => a,
        (None, b) => b,
    }
}

/// The samples of one window, folded.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Accumulator {
    samples: u64,
    gpu: Mean,
    encoder: Mean,
    decoder: Mean,
    pss_peak: Option<f64>,
    cpu: Mean,
    cpu_peak: Option<f64>,
    hot: Mean,
}

impl Accumulator {
    pub fn add(&mut self, sample: &Sample) {
        self.samples += 1;
        self.gpu.add(sample.gpu_pct);
        self.encoder.add(sample.encoder_pct);
        self.decoder.add(sample.decoder_pct);
        self.pss_peak = peak(self.pss_peak, sample.pss_mib);
        self.cpu.add(sample.cpu_cores);
        self.cpu_peak = peak(self.cpu_peak, sample.cpu_cores);
        self.hot.add(sample.hot_thread_pct);
    }

    /// How many samples the window took.
    pub fn samples(&self) -> u64 {
        self.samples
    }

    /// The window's use.
    pub fn usage(&self) -> StepUse {
        StepUse {
            gpu_busy_pct: self.gpu.value(),
            gpu_encoder_pct: self.encoder.value(),
            gpu_decoder_pct: self.decoder.value(),
            job_ram_mib: self.pss_peak,
            cpu_cores_mean: self.cpu.value(),
            cpu_cores_peak: self.cpu_peak,
            busiest_thread_pct: self.hot.value(),
        }
    }
}

/// What the job used while one step ran; `None` where no sample measured it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct StepUse {
    pub gpu_busy_pct: Option<f64>,
    pub gpu_encoder_pct: Option<f64>,
    pub gpu_decoder_pct: Option<f64>,
    pub job_ram_mib: Option<f64>,
    pub cpu_cores_mean: Option<f64>,
    pub cpu_cores_peak: Option<f64>,
    pub busiest_thread_pct: Option<f64>,
}

impl StepUse {
    /// Put this use into `measure`'s fields of the same names.
    pub fn apply(&self, measure: &mut StepMeasure) {
        measure.gpu_busy_pct = self.gpu_busy_pct;
        measure.gpu_encoder_pct = self.gpu_encoder_pct;
        measure.gpu_decoder_pct = self.gpu_decoder_pct;
        measure.job_ram_mib = self.job_ram_mib;
        measure.cpu_cores_mean = self.cpu_cores_mean;
        measure.cpu_cores_peak = self.cpu_cores_peak;
        measure.busiest_thread_pct = self.busiest_thread_pct;
    }
}

/// The windows open now, one per running step, and the whole job's.
#[derive(Debug, Clone, Default)]
pub struct Windows {
    open: BTreeMap<StepName, Accumulator>,
    job: Accumulator,
}

impl Windows {
    /// Start `step`'s window, empty; a window already open for it starts over.
    pub fn open(&mut self, step: StepName) {
        self.open.insert(step, Accumulator::default());
    }

    /// End `step`'s window and return its use; a step with no window open used nothing measured.
    pub fn close(&mut self, step: StepName) -> StepUse {
        self.open
            .remove(&step)
            .map(|window| window.usage())
            .unwrap_or_default()
    }

    /// Add `sample` to every open window and to the job's.
    pub fn add(&mut self, sample: &Sample) {
        self.job.add(sample);
        for window in self.open.values_mut() {
            window.add(sample);
        }
    }

    /// The whole job's samples, folded.
    pub fn job(&self) -> &Accumulator {
        &self.job
    }
}

#[cfg(test)]
#[path = "tests/step_use.rs"]
mod tests;
