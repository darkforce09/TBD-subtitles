//! Numbers as the window writes them: sizes, durations, video lengths and times, places in line
//! and counts.
//!
//! **Role:** turn sizes, seconds and counts into the words every view shows.
//!
//! **Position:** used by the features' views; depends on nothing.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** a negative time reads as zero.

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

/// A time still to wait, loosely: `about 4 min`, `under 2 min` or `a few seconds`.
pub(crate) fn about(seconds: f64) -> String {
    if seconds >= 90.0 {
        format!("about {} min", (seconds / 60.0).round() as u64)
    } else if seconds >= 20.0 {
        "under 2 min".to_string()
    } else {
        "a few seconds".to_string()
    }
}

/// A video's length as `25:59`, or `1:02:03` from an hour on.
pub(crate) fn length(seconds: f64) -> String {
    let s = seconds.max(0.0).round() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// A video time as `h:mm:ss`.
pub(crate) fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0).floor() as u64;
    format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// A place in line: `1st`, `2nd`, `3rd`, `4th`, `11th`, `21st`.
pub(crate) fn ordinal(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// A count with its noun: `1 correction`, `2 corrections`, `0 corrections`.
pub(crate) fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

#[cfg(test)]
#[path = "tests/format.rs"]
mod tests;
