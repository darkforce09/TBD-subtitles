//! Which filesystem a folder lives on, read from `/proc/self/mountinfo`.
//!
//! **Role:** names the filesystem in the matrix header, because file locks behave differently
//! on local, overlay and network filesystems.
//!
//! **Position:** used by `matrix`; depends on the standard library only.
//!
//! **Signals and state:** reads `/proc/self/mountinfo` once per call.
//!
//! **Invariants:** the mount chosen is the one whose mount point is the longest whole-component
//! prefix of the path; when two mounts share that point, the later line (the one on top) wins;
//! an unreadable mount table gives `unknown`, never a guess.

use std::path::Path;

/// One line of the mount table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mount {
    pub(crate) mount_point: String,
    pub(crate) fs_type: String,
    pub(crate) source: String,
}

/// Every mount in a `/proc/self/mountinfo` text; lines that do not parse are skipped.
pub(crate) fn parse_mountinfo(text: &str) -> Vec<Mount> {
    text.lines().filter_map(parse_line).collect()
}

/// One mountinfo line: `id parent major:minor root mount_point options [optional…] - type
/// source super_options`.
fn parse_line(line: &str) -> Option<Mount> {
    let (before, after) = line.split_once(" - ")?;
    let mount_point = before.split(' ').nth(4)?;
    let mut after = after.split(' ');
    let fs_type = after.next()?;
    let source = after.next()?;
    Some(Mount {
        mount_point: unescape(mount_point),
        fs_type: unescape(fs_type),
        source: unescape(source),
    })
}

/// Undoes the kernel's octal escapes (`\040` for a space, `\011`, `\012`, `\134`).
fn unescape(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let octal = bytes.get(index + 1..index + 4).filter(|digits| {
            bytes[index] == b'\\' && digits.iter().all(|digit| (b'0'..=b'7').contains(digit))
        });
        match octal {
            Some(digits) => {
                let value = digits
                    .iter()
                    .fold(0u32, |value, digit| value * 8 + u32::from(digit - b'0'));
                out.push(u8::try_from(value).unwrap_or(b'?'));
                index += 4;
            }
            None => {
                out.push(bytes[index]);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The mount `path` lives on: the longest mount point that is a whole-component prefix of it.
pub(crate) fn mount_of<'a>(path: &Path, mounts: &'a [Mount]) -> Option<&'a Mount> {
    let mut best: Option<&Mount> = None;
    for mount in mounts {
        if !path.starts_with(&mount.mount_point) {
            continue;
        }
        let longer = best.is_none_or(|best| mount.mount_point.len() >= best.mount_point.len());
        if longer {
            best = Some(mount);
        }
    }
    best
}

/// `<type> (<source> at <mount point>)` for the folder, or `unknown (<reason>)`.
pub(crate) fn describe(path: &Path) -> String {
    let path = match path.canonicalize() {
        Ok(path) => path,
        Err(error) => return format!("unknown ({error})"),
    };
    let text = match std::fs::read_to_string("/proc/self/mountinfo") {
        Ok(text) => text,
        Err(error) => return format!("unknown (/proc/self/mountinfo: {error})"),
    };
    let mounts = parse_mountinfo(&text);
    match mount_of(&path, &mounts) {
        Some(mount) => format!(
            "{} ({} at {})",
            mount.fs_type, mount.source, mount.mount_point
        ),
        None => "unknown (no mount matches)".to_string(),
    }
}

#[cfg(test)]
#[path = "tests/filesystem.rs"]
mod tests;
