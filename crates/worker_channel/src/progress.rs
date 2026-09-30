//! The payload of a `Progress` frame: how many of a step's units are done, out of how many.
//!
//! **Role:** encode and decode the two counts as two little-endian `u64`s, 16 bytes.
//!
//! **Position:** sent by `worker::progress`; decoded by the job runner into its own progress
//! event.
//!
//! **Signals and state:** none; plain data.
//!
//! **Invariants:** a payload of any length but 16 bytes is `InvalidData`.

use std::io;

/// The bytes of an encoded [`Progress`].
pub const ENCODED_LEN: usize = 16;

/// How far a step is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
}

impl Progress {
    /// The frame payload: `done`, then `total`.
    pub fn encode(&self) -> [u8; ENCODED_LEN] {
        let mut bytes = [0u8; ENCODED_LEN];
        bytes[..8].copy_from_slice(&self.done.to_le_bytes());
        bytes[8..].copy_from_slice(&self.total.to_le_bytes());
        bytes
    }

    /// The progress a frame payload carries.
    pub fn decode(bytes: &[u8]) -> io::Result<Progress> {
        let (Some(done), Some(total)) = (bytes.get(..8), bytes.get(8..)) else {
            return Err(wrong_length(bytes.len()));
        };
        let (Ok(done), Ok(total)) = (<[u8; 8]>::try_from(done), <[u8; 8]>::try_from(total)) else {
            return Err(wrong_length(bytes.len()));
        };
        Ok(Progress {
            done: u64::from_le_bytes(done),
            total: u64::from_le_bytes(total),
        })
    }
}

fn wrong_length(len: usize) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("a progress payload of {len} bytes, not {ENCODED_LEN}"),
    )
}

#[cfg(test)]
#[path = "tests/progress.rs"]
mod tests;
