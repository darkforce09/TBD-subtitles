//! Where an input or output value lives in the job database: a table and a key.
//!
//! **Role:** name the job database's tables and encode the address that opens every `Input` and
//! `Output` payload: the table id, the key kind, the key text's `u16` little-endian length and
//! bytes, and for a per-frame key the frame number as a `u64` little-endian.
//!
//! **Position:** used by `worker` to address an output and to read its inputs, and by the job
//! runner to route both; the archive after the address belongs to `job_model`'s types.
//!
//! **Signals and state:** none; plain data.
//!
//! **Invariants:** the archive's length is the payload's length minus the address's; a table id,
//! key kind or key text that does not decode is `InvalidData`, never a guess; a key text over
//! `u16::MAX` bytes is refused when encoding.

use std::fmt;
use std::io::{self, Read};
use std::str::FromStr;

/// The key kind byte of a named key.
const KIND_NAME: u8 = 0;
/// The key kind byte of a per-frame key.
const KIND_FRAME: u8 = 1;

/// A table of the job database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Table {
    Meta,
    StepRecords,
    Outputs,
    Corrections,
    Frames,
    Readings,
}

impl Table {
    /// Every table, in id order.
    pub const ALL: [Table; 6] = [
        Table::Meta,
        Table::StepRecords,
        Table::Outputs,
        Table::Corrections,
        Table::Frames,
        Table::Readings,
    ];

    /// The table's byte on the wire.
    pub fn id(self) -> u8 {
        match self {
            Table::Meta => 1,
            Table::StepRecords => 2,
            Table::Outputs => 3,
            Table::Corrections => 4,
            Table::Frames => 5,
            Table::Readings => 6,
        }
    }

    /// The table's name in the job database.
    pub fn name(self) -> &'static str {
        match self {
            Table::Meta => "meta",
            Table::StepRecords => "step_records",
            Table::Outputs => "outputs",
            Table::Corrections => "corrections",
            Table::Frames => "frames",
            Table::Readings => "readings",
        }
    }

    /// The table whose byte is `id`.
    pub fn from_id(id: u8) -> Option<Table> {
        Table::ALL.into_iter().find(|table| table.id() == id)
    }
}

impl fmt::Display for Table {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Table {
    type Err = String;

    fn from_str(text: &str) -> Result<Table, String> {
        Table::ALL
            .into_iter()
            .find(|table| table.name() == text)
            .ok_or_else(|| format!("`{text}` is not a table of the job database"))
    }
}

/// A row's key within its table.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
    /// A key by name, such as a step's name.
    Name(String),
    /// One frame of one on-screen text occurrence.
    Frame { occurrence: String, frame: u64 },
}

/// A table and a key: one value of the job database.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Address {
    pub table: Table,
    pub key: Key,
}

impl Address {
    /// The address's bytes, as they open an `Input` or `Output` payload.
    pub fn encode(&self) -> io::Result<Vec<u8>> {
        let (kind, text, frame) = match &self.key {
            Key::Name(name) => (KIND_NAME, name, None),
            Key::Frame { occurrence, frame } => (KIND_FRAME, occurrence, Some(*frame)),
        };
        let len = u16::try_from(text.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "a key of {} bytes passes the key limit of {} bytes",
                    text.len(),
                    u16::MAX
                ),
            )
        })?;
        let mut bytes = Vec::with_capacity(4 + text.len() + 8);
        bytes.push(self.table.id());
        bytes.push(kind);
        bytes.extend_from_slice(&len.to_le_bytes());
        bytes.extend_from_slice(text.as_bytes());
        if let Some(frame) = frame {
            bytes.extend_from_slice(&frame.to_le_bytes());
        }
        Ok(bytes)
    }

    /// Read an address from the front of `r`, with the number of bytes it took.
    pub fn read(r: &mut impl Read) -> io::Result<(Address, usize)> {
        let mut front = [0u8; 4];
        r.read_exact(&mut front)?;
        let [table, kind, a, b] = front;
        let table = Table::from_id(table)
            .ok_or_else(|| invalid(format!("an address with the unknown table id {table}")))?;
        let len = usize::from(u16::from_le_bytes([a, b]));
        let mut text = vec![0u8; len];
        r.read_exact(&mut text)?;
        let text = String::from_utf8(text)
            .map_err(|_| invalid("an address whose key is not UTF-8".to_string()))?;
        match kind {
            KIND_NAME => Ok((
                Address {
                    table,
                    key: Key::Name(text),
                },
                4 + len,
            )),
            KIND_FRAME => {
                let mut frame = [0u8; 8];
                r.read_exact(&mut frame)?;
                Ok((
                    Address {
                        table,
                        key: Key::Frame {
                            occurrence: text,
                            frame: u64::from_le_bytes(frame),
                        },
                    },
                    4 + len + 8,
                ))
            }
            other => Err(invalid(format!(
                "an address with the unknown key kind {other}"
            ))),
        }
    }
}

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
#[path = "tests/address.rs"]
mod tests;
