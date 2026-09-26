//! Numbers as the window writes them.

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

#[cfg(test)]
#[path = "tests/format.rs"]
mod tests;
