//! The read end of FFmpeg's frame pipe, enlarged so a full-resolution frame crosses it in few
//! reads.
//!
//! **Role:** raise a pipe's capacity to 1 MiB with `fcntl(F_SETPIPE_SZ)`, never past the
//! system's `pipe-max-size`, and read a pipe's capacity back.
//! **Position:** used by `yuv_stream` on FFmpeg's stdout, and by the detection benchmark's decode
//! routes.
//! **Signals and state:** reads `/proc/sys/fs/pipe-max-size`; changes only the pipe's capacity.
//! **Invariants:** the size asked for is at most 1 MiB and at most `pipe-max-size`, so an
//! unprivileged process is never refused for asking past the limit; the pipe's data is untouched.

use std::os::fd::AsRawFd;

use crate::MediaError;

/// The capacity asked for: 1 MiB, against Linux's default of 64 KiB.
pub const WIDE_PIPE_BYTES: usize = 1 << 20;

/// Where Linux publishes the largest capacity an unprivileged process may set.
const PIPE_MAX_SIZE: &str = "/proc/sys/fs/pipe-max-size";

/// The capacity [`enlarge`] asks for: [`WIDE_PIPE_BYTES`], or `pipe-max-size` when smaller.
pub fn wide_pipe_bytes() -> usize {
    clamp_to_max(std::fs::read_to_string(PIPE_MAX_SIZE).ok().as_deref())
}

/// [`WIDE_PIPE_BYTES`], lowered to the limit `max_size` reads as, if it reads as a positive size.
fn clamp_to_max(max_size: Option<&str>) -> usize {
    max_size
        .and_then(|text| text.trim().parse::<usize>().ok())
        .filter(|max| *max > 0)
        .map_or(WIDE_PIPE_BYTES, |max| max.min(WIDE_PIPE_BYTES))
}

/// Raise `pipe`'s capacity to [`wide_pipe_bytes`]; the capacity afterwards.
pub fn enlarge(pipe: &impl AsRawFd) -> Result<usize, MediaError> {
    let fd = pipe.as_raw_fd();
    let bytes = libc::c_int::try_from(wide_pipe_bytes()).unwrap_or(libc::c_int::MAX);
    // SAFETY: `fd` is an open pipe end owned by `pipe`, which outlives the call; F_SETPIPE_SZ
    // changes only its capacity.
    let set = unsafe { libc::fcntl(fd, libc::F_SETPIPE_SZ, bytes) };
    if set < 0 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EPERM) {
            return capacity(pipe);
        }
        return Err(MediaError::Parse(format!("F_SETPIPE_SZ: {err}")));
    }
    capacity(pipe)
}

/// `pipe`'s capacity in bytes.
pub fn capacity(pipe: &impl AsRawFd) -> Result<usize, MediaError> {
    // SAFETY: as in `enlarge`; F_GETPIPE_SZ only reads the capacity.
    let size = unsafe { libc::fcntl(pipe.as_raw_fd(), libc::F_GETPIPE_SZ) };
    usize::try_from(size).map_err(|_| {
        MediaError::Parse(format!("F_GETPIPE_SZ: {}", std::io::Error::last_os_error()))
    })
}

#[cfg(test)]
#[path = "tests/pipe.rs"]
mod tests;
