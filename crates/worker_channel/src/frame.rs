//! The frame codec: one tag byte, a little-endian `u32` payload length, then the payload.
//!
//! **Role:** write a frame from parts and read one back, header and payload apart, so a reader can
//! decide where the payload goes before it reads it.
//!
//! **Position:** used by `worker` to send and by the job runner to read; the payload's meaning
//! belongs to the tag's owner (`address`, `progress`, or the caller).
//!
//! **Signals and state:** none; reads and writes the stream it is given.
//!
//! **Invariants:** a written frame's length is the sum of its parts and never passes `u32::MAX`;
//! [`read_header`] answers `Ok(None)` only for a stream that ends cleanly before a frame starts,
//! `UnexpectedEof` for one that ends inside a header or payload, `InvalidData` for an unknown
//! tag; a payload is allocated as its bytes arrive, so a false length cannot claim memory the
//! stream does not deliver.

use std::io::{self, Read, Write};

/// The bytes of a frame header: the tag and the length.
pub const HEADER_LEN: usize = 5;

/// What a frame carries.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tag {
    /// A value the step reads: an address, then one `rkyv` archive. Runner to worker.
    Input = 1,
    /// A value the step writes: an address, then one `rkyv` archive. Worker to runner.
    Output = 2,
    /// How far the step is: a [`crate::progress::Progress`].
    Progress = 3,
    /// One language-model call, as the JSON of the app's model-call record.
    ModelCall = 4,
    /// The worker's measure of itself, as one `rkyv` archive.
    Measure = 5,
    /// Why the step failed, as UTF-8 text.
    Failed = 6,
    /// The step finished; the payload is empty.
    Done = 7,
}

impl Tag {
    /// Every tag, in wire order.
    pub const ALL: [Tag; 7] = [
        Tag::Input,
        Tag::Output,
        Tag::Progress,
        Tag::ModelCall,
        Tag::Measure,
        Tag::Failed,
        Tag::Done,
    ];

    /// The tag's name in messages, such as `model call`.
    pub fn name(self) -> &'static str {
        match self {
            Tag::Input => "input",
            Tag::Output => "output",
            Tag::Progress => "progress",
            Tag::ModelCall => "model call",
            Tag::Measure => "measure",
            Tag::Failed => "failure",
            Tag::Done => "end",
        }
    }
}

impl TryFrom<u8> for Tag {
    /// The byte that names no tag.
    type Error = u8;

    fn try_from(byte: u8) -> Result<Tag, u8> {
        Tag::ALL
            .into_iter()
            .find(|tag| *tag as u8 == byte)
            .ok_or(byte)
    }
}

/// The front of a frame: what it carries and how many payload bytes follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub tag: Tag,
    pub len: u32,
}

/// A whole frame, read into memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub tag: Tag,
    pub payload: Vec<u8>,
}

/// Write one frame whose payload is `parts` joined, then flush.
pub fn write_frame(w: &mut impl Write, tag: Tag, parts: &[&[u8]]) -> io::Result<()> {
    let total: usize = parts.iter().map(|part| part.len()).sum();
    let len = u32::try_from(total).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "a {} frame of {total} bytes passes the frame limit of {} bytes",
                tag.name(),
                u32::MAX
            ),
        )
    })?;
    let [a, b, c, d] = len.to_le_bytes();
    w.write_all(&[tag as u8, a, b, c, d])?;
    for part in parts {
        w.write_all(part)?;
    }
    w.flush()
}

/// Read the next frame's header; `Ok(None)` when the stream ends cleanly before it.
pub fn read_header(r: &mut impl Read) -> io::Result<Option<Header>> {
    let mut bytes = [0u8; HEADER_LEN];
    let mut filled = 0;
    while filled < HEADER_LEN {
        match r.read(&mut bytes[filled..]) {
            Ok(0) if filled == 0 => return Ok(None),
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    format!(
                        "the stream ends inside a frame header, after {filled} of {HEADER_LEN} bytes"
                    ),
                ));
            }
            Ok(n) => filled += n,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    let [tag, a, b, c, d] = bytes;
    let tag = Tag::try_from(tag).map_err(|byte| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("a frame with the unknown tag {byte}"),
        )
    })?;
    Ok(Some(Header {
        tag,
        len: u32::from_le_bytes([a, b, c, d]),
    }))
}

/// Read a payload of `len` bytes.
pub fn read_payload(r: &mut impl Read, len: u32) -> io::Result<Vec<u8>> {
    let mut payload = Vec::new();
    r.take(u64::from(len)).read_to_end(&mut payload)?;
    if payload.len() < len as usize {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!(
                "the stream ends inside a frame payload, after {} of {len} bytes",
                payload.len()
            ),
        ));
    }
    Ok(payload)
}

/// Read the next whole frame; `Ok(None)` when the stream ends cleanly before it.
pub fn read_frame(r: &mut impl Read) -> io::Result<Option<Frame>> {
    let Some(header) = read_header(r)? else {
        return Ok(None);
    };
    let payload = read_payload(r, header.len)?;
    Ok(Some(Frame {
        tag: header.tag,
        payload,
    }))
}

#[cfg(test)]
#[path = "tests/frame.rs"]
mod tests;
