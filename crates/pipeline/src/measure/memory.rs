//! Peak resident memory of this process and of the children it waited for.

use std::fs;

/// `VmHWM` from `/proc/self/status`, in MiB; `None` when it cannot be read.
pub fn peak_ram_mib() -> Option<f64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let kib = status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|n| n.parse::<f64>().ok())?;
    Some(kib / 1024.0)
}

/// `ru_maxrss` of the children this process waited for (FFmpeg, `claude`), in MiB; `None` when
/// it cannot be read or no child ran.
pub fn peak_child_ram_mib() -> Option<f64> {
    // SAFETY: getrusage writes one plain struct that is zeroed first.
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    let ok = unsafe { libc::getrusage(libc::RUSAGE_CHILDREN, &mut usage) } == 0;
    (ok && usage.ru_maxrss > 0).then(|| usage.ru_maxrss as f64 / 1024.0)
}

/// Reset this process's `VmHWM` to its current size (Linux `clear_refs` 5), so the next
/// reading is the peak of what runs next; `false` when the kernel refused.
pub fn reset_peak_ram() -> bool {
    fs::write("/proc/self/clear_refs", "5").is_ok()
}
