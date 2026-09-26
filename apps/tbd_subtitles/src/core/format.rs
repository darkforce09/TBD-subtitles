//! Numbers as the window writes them: sizes and durations.

/// Bytes as MiB below one GiB, else GiB, with one decimal.
pub(crate) fn size(bytes: u64) -> String {
    const MIB: f64 = 1_048_576.0;
    let mib = bytes as f64 / MIB;
    if mib < 1024.0 {
        format!("{mib:.1} MiB")
    } else {
        format!("{:.1} GiB", mib / 1024.0)
    }
}

/// Seconds as `1 h 02 min`, `4 min 05 s` or `12 s`.
pub(crate) fn duration(seconds: f64) -> String {
    let s = seconds.max(0.0).round() as u64;
    if s >= 3600 {
        format!("{} h {:02} min", s / 3600, s / 60 % 60)
    } else if s >= 60 {
        format!("{} min {:02} s", s / 60, s % 60)
    } else {
        format!("{s} s")
    }
}

#[cfg(test)]
#[path = "tests/format.rs"]
mod tests;
