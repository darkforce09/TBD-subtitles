//! Peak VRAM of one worker process, sampled through NVML while it runs.
//!
//! **Role:** describe the device (name, driver, memory), report its free memory before a GPU
//! step, and sample the worker pid's own VRAM and the device's use above the baseline every
//! 100 ms while it runs.
//!
//! **Position:** called by `crate::workers` around each GPU worker, by the app's machine check,
//! and by the stack spike tool;
//! loads NVML, the driver's own library (`libnvidia-ml.so`), at run time, so no `nvidia-smi`
//! child is needed.
//!
//! **Signals and state:** one sampling thread per worker, stopped by an atomic flag.
//!
//! **Invariants:** NVML exists on the host and not in the development container; without it the
//! monitor reports that VRAM was not measured, never a zero.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use nvml_wrapper::Nvml;
use nvml_wrapper::enums::device::UsedGpuMemory;

const SAMPLE_EVERY: Duration = Duration::from_millis(100);
const MIB: u64 = 1 << 20;

/// Free and used device memory right now, in MiB.
pub struct DeviceMemory {
    pub used_mib: u64,
    pub free_mib: u64,
}

/// What the sampler saw over a worker's life.
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct VramPeaks {
    /// The worker pid's own allocation, as NVML reports it per process.
    pub process_mib: u64,
    /// Device memory used above what was in use before the worker started.
    pub device_delta_mib: u64,
    pub samples: u64,
}

/// The device's memory now, or `None` when NVML cannot be loaded.
pub fn device_memory() -> Option<DeviceMemory> {
    let nvml = Nvml::init().ok()?;
    let info = nvml.device_by_index(0).ok()?.memory_info().ok()?;
    Some(DeviceMemory {
        used_mib: info.used / MIB,
        free_mib: info.free / MIB,
    })
}

/// The first GPU as the driver describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub name: String,
    pub driver: String,
    pub total_mib: u64,
    pub free_mib: u64,
}

/// The first GPU's name, the driver's version and the device's memory, or `None` when NVML
/// cannot be loaded or finds no device.
pub fn device_info() -> Option<DeviceInfo> {
    let nvml = Nvml::init().ok()?;
    let device = nvml.device_by_index(0).ok()?;
    let memory = device.memory_info().ok()?;
    Some(DeviceInfo {
        name: device.name().unwrap_or_else(|_| "unknown GPU".to_string()),
        driver: nvml
            .sys_driver_version()
            .unwrap_or_else(|_| "unknown".to_string()),
        total_mib: memory.total / MIB,
        free_mib: memory.free / MIB,
    })
}

/// A sampling thread watching one pid.
pub struct Monitor {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<Option<VramPeaks>>,
}

impl Monitor {
    /// Start sampling `pid` against the device's use at `baseline_mib`.
    pub fn start(pid: u32, baseline_mib: u64) -> Monitor {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::spawn(move || {
            let nvml = Nvml::init().ok()?;
            let device = nvml.device_by_index(0).ok()?;
            let mut peaks = VramPeaks::default();
            while !flag.load(Ordering::SeqCst) {
                if let Ok(info) = device.memory_info() {
                    let delta = (info.used / MIB).saturating_sub(baseline_mib);
                    peaks.device_delta_mib = peaks.device_delta_mib.max(delta);
                }
                if let Ok(processes) = device.running_compute_processes() {
                    for p in processes.iter().filter(|p| p.pid == pid) {
                        if let UsedGpuMemory::Used(bytes) = p.used_gpu_memory {
                            peaks.process_mib = peaks.process_mib.max(bytes / MIB);
                        }
                    }
                }
                peaks.samples += 1;
                std::thread::sleep(SAMPLE_EVERY);
            }
            Some(peaks)
        });
        Monitor { stop, thread }
    }

    /// Stop sampling; `None` when NVML was never available.
    pub fn finish(self) -> Option<VramPeaks> {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.join().ok().flatten()
    }
}
