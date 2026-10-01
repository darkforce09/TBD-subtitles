//! Whether the copied pieces of a joined video hold the source's packets unchanged.
//!
//! **Role:** hash every video packet of the source and of the joined file twice, as stored and
//! with the SPS and PPS units removed, and compare the packets of every copied piece.
//! **Position:** inside `encode::segments`; the last of the join checks in `verify`.
//! **Signals and state:** one FFmpeg run per file, stream-copying the first video stream into two
//! `framemd5` lists in the caller's folder, which are read and removed.
//! **Invariants:** packets compare in decode order, which matches presentation order at every
//! piece's IDR boundary; a copied packet passes when its bytes are the source's, or when they
//! are longer and equal the source's once SPS and PPS units are removed from both, which is all
//! the copy's in-band headers add.

use std::path::{Path, PathBuf};

use child_process::Run;

use super::FallbackReason;
use super::args::remove_if_present;
use super::plan::Piece;
use super::verify::VerifyRequest;
use crate::encode::path;
use crate::{MediaError, Programs};

/// The bit stream filter that removes SPS (7) and PPS (8) units from every packet.
const WITHOUT_PARAMETER_SETS: &str = "filter_units=remove_types=7|8";

/// One packet's size and MD5 as `framemd5` lists them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PacketHash {
    pub size: u64,
    pub md5: String,
}

/// A file's packet hashes as stored and without parameter sets, in decode order.
struct Hashes {
    stored: Vec<PacketHash>,
    without_headers: Vec<PacketHash>,
}

/// Every copied piece's packets in the joined file are the source's, apart from in-band headers.
pub(super) fn copied_packets_match(
    programs: &Programs,
    request: &VerifyRequest,
) -> Result<(), FallbackReason> {
    if !request.pieces.iter().any(|piece| piece.is_copy()) {
        return Ok(());
    }
    let unreadable = |error: MediaError| {
        FallbackReason(format!("the copied packets could not be compared: {error}"))
    };
    let source = hashes(programs, request, request.source, "source").map_err(unreadable)?;
    let joined = hashes(programs, request, request.output, "joined").map_err(unreadable)?;
    compare_copied(request.pieces, &source, &joined)
}

fn compare_copied(
    pieces: &[Piece],
    source: &Hashes,
    joined: &Hashes,
) -> Result<(), FallbackReason> {
    for piece in pieces.iter().filter(|piece| piece.is_copy()) {
        for index in piece.first()..=piece.last() {
            let at = index as usize;
            let same = match (
                source.stored.get(at),
                joined.stored.get(at),
                source.without_headers.get(at),
                joined.without_headers.get(at),
            ) {
                (
                    Some(source_packet),
                    Some(joined_packet),
                    Some(source_bare),
                    Some(joined_bare),
                ) => packet_kept(source_packet, joined_packet, source_bare, joined_bare),
                _ => false,
            };
            if !same {
                return Err(FallbackReason(format!(
                    "packet {index} of the copied frames {}..={} differs from the source's",
                    piece.first(),
                    piece.last()
                )));
            }
        }
    }
    Ok(())
}

/// A copied packet is kept when its bytes are the source's, or longer with only parameter sets
/// added.
pub(super) fn packet_kept(
    source: &PacketHash,
    joined: &PacketHash,
    source_bare: &PacketHash,
    joined_bare: &PacketHash,
) -> bool {
    source == joined || (joined.size > source.size && source_bare == joined_bare)
}

/// Hash `video`'s first video stream both ways into the request's folder and read the lists.
fn hashes(
    programs: &Programs,
    request: &VerifyRequest,
    video: &Path,
    name: &str,
) -> Result<Hashes, MediaError> {
    let stored_list = request.folder.join(format!("{name}.framemd5"));
    let bare_list = request.folder.join(format!("{name}.bare.framemd5"));
    let output = Run::new(&programs.ffmpeg)
        .args(hash_args(video, &stored_list, &bare_list))
        .timeout(request.timeout)
        .output()?;
    if output.code != 0 {
        return Err(MediaError::Exit {
            program: programs.ffmpeg.clone(),
            code: output.code,
            stderr: output.stderr,
        });
    }
    let hashes = Hashes {
        stored: parse_framemd5(&read(&stored_list)?),
        without_headers: parse_framemd5(&read(&bare_list)?),
    };
    remove_if_present(&stored_list)?;
    remove_if_present(&bare_list)?;
    Ok(hashes)
}

fn read(list: &PathBuf) -> Result<String, MediaError> {
    std::fs::read_to_string(list)
        .map_err(|error| MediaError::Parse(format!("cannot read {}: {error}", list.display())))
}

/// FFmpeg's arguments to stream-copy the first video stream of `video` into two `framemd5`
/// lists: as stored, and with SPS and PPS units removed.
pub(super) fn hash_args(video: &Path, stored_list: &Path, bare_list: &Path) -> Vec<String> {
    let mut args: Vec<String> = ["-nostdin", "-hide_banner", "-v", "error", "-y", "-i"]
        .map(String::from)
        .to_vec();
    args.push(path(video));
    args.extend(["-map", "0:v:0", "-c", "copy", "-f", "framemd5"].map(String::from));
    args.push(path(stored_list));
    args.extend(
        [
            "-map",
            "0:v:0",
            "-c",
            "copy",
            "-bsf:v",
            WITHOUT_PARAMETER_SETS,
        ]
        .map(String::from),
    );
    args.extend(["-f", "framemd5"].map(String::from));
    args.push(path(bare_list));
    args
}

/// The packets of a `framemd5` list in order: each line not starting with `#` holds the stream,
/// dts, pts, duration, size and MD5, separated by commas.
pub(super) fn parse_framemd5(list: &str) -> Vec<PacketHash> {
    list.lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let fields: Vec<&str> = line.split(',').map(str::trim).collect();
            Some(PacketHash {
                size: fields.get(4)?.parse().ok()?,
                md5: fields.get(5)?.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/copied.rs"]
mod tests;
