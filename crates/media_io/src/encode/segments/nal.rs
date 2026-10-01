//! H.264 NAL units inside one packet, read in pure Rust.
//!
//! **Role:** list the NAL unit types of a packet stored either with start codes (Annex B, as in
//! MPEG-TS) or with big-endian length prefixes (AVC, as in Matroska and MP4), tell whether the
//! packet opens an IDR picture, and read the packet bytes ffprobe prints with `-show_data`.
//! **Position:** inside `encode::segments`; the IDR probe reads keyframe packets through it.
//! **Signals and state:** pure functions over byte slices and text; holds nothing.
//! **Invariants:** a malformed packet (no start code, a length past the end, an empty unit, a set
//! forbidden bit) has no unit list, and so never counts as an IDR picture; only the first slice
//! of the packet decides, so SEI, SPS and PPS units before it are allowed.

/// The NAL unit type of a slice of a non-IDR picture.
pub const NAL_SLICE: u8 = 1;
/// The NAL unit type of a slice of an IDR picture: every later picture decodes without any
/// picture before it.
pub const NAL_IDR_SLICE: u8 = 5;
/// The NAL unit type of a sequence parameter set.
pub const NAL_SPS: u8 = 7;
/// The NAL unit type of a picture parameter set.
pub const NAL_PPS: u8 = 8;

/// How a packet stores its NAL units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketFormat {
    /// Each unit follows a `00 00 01` or `00 00 00 01` start code.
    AnnexB,
    /// Each unit follows its length as a big-endian number of this many bytes (1 to 4).
    LengthPrefixed(u8),
}

/// The type of every NAL unit in `packet`, in stored order; `None` when the packet is malformed.
pub fn nal_unit_types(packet: &[u8], format: PacketFormat) -> Option<Vec<u8>> {
    let headers = match format {
        PacketFormat::AnnexB => annex_b_headers(packet)?,
        PacketFormat::LengthPrefixed(size) => length_prefixed_headers(packet, size)?,
    };
    headers
        .into_iter()
        .map(|header| (header & 0x80 == 0).then_some(header & 0x1F))
        .collect()
}

/// Whether `packet` opens an IDR picture: its first slice unit is an IDR slice.
pub fn starts_idr_picture(packet: &[u8], format: PacketFormat) -> bool {
    nal_unit_types(packet, format)
        .and_then(|types| {
            types
                .into_iter()
                .find(|&kind| kind == NAL_SLICE || kind == NAL_IDR_SLICE)
        })
        .is_some_and(|kind| kind == NAL_IDR_SLICE)
}

/// The header byte of every unit after a start code. Anything before the first start code must
/// be zero bytes; a start code at the very end is malformed.
fn annex_b_headers(packet: &[u8]) -> Option<Vec<u8>> {
    let mut headers = Vec::new();
    let mut zeros = 0usize;
    let mut position = 0;
    while position < packet.len() {
        let byte = packet[position];
        if byte == 1 && zeros >= 2 {
            headers.push(*packet.get(position + 1)?);
            zeros = 0;
        } else if byte == 0 {
            zeros += 1;
        } else {
            if headers.is_empty() {
                return None;
            }
            zeros = 0;
        }
        position += 1;
    }
    (!headers.is_empty()).then_some(headers)
}

/// The header byte of every length-prefixed unit; a length past the end, an empty unit or a
/// prefix size outside 1 to 4 is malformed.
fn length_prefixed_headers(packet: &[u8], size: u8) -> Option<Vec<u8>> {
    let size = usize::from(size);
    if !(1..=4).contains(&size) {
        return None;
    }
    let mut headers = Vec::new();
    let mut position = 0;
    while position < packet.len() {
        let prefix = packet.get(position..position + size)?;
        let length = prefix
            .iter()
            .fold(0usize, |length, &byte| (length << 8) | usize::from(byte));
        let start = position + size;
        if length == 0 || start + length > packet.len() {
            return None;
        }
        headers.push(packet[start]);
        position = start + length;
    }
    (!headers.is_empty()).then_some(headers)
}

/// The bytes of an ffprobe `-show_data` hex dump: lines of an eight-digit offset, a colon, up to
/// sixteen bytes as hex in pairs, and their printable text. `None` when a line does not follow
/// that layout or an offset is out of sequence.
pub fn parse_hex_dump<'a>(lines: impl IntoIterator<Item = &'a str>) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    for line in lines {
        let (offset, rest) = line.split_once(": ")?;
        if offset.len() != 8 || usize::from_str_radix(offset, 16).ok()? != bytes.len() {
            return None;
        }
        // The hex columns take 41 characters; the printable text after them may hold anything.
        let columns = rest.get(..41.min(rest.len()))?;
        let before = bytes.len();
        for group in columns.split_whitespace() {
            if group.len() % 2 != 0 {
                return None;
            }
            for pair in group.as_bytes().chunks(2) {
                let pair = std::str::from_utf8(pair).ok()?;
                bytes.push(u8::from_str_radix(pair, 16).ok()?);
            }
        }
        if bytes.len() - before > 16 || bytes.len() == before {
            return None;
        }
    }
    Some(bytes)
}

#[cfg(test)]
#[path = "tests/nal.rs"]
mod tests;
