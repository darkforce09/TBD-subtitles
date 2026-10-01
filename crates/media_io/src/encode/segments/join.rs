//! The copied pieces cut from the source, and the join of every piece with the source's audio.
//!
//! **Role:** cut the source's video into its copied pieces in one FFmpeg run, name each piece's
//! file, write the concat list of every piece with its exact duration, and run the join that
//! muxes the joined video with the source's audio, chapters and metadata into Matroska.
//! **Position:** inside `encode::segments`; the localize stage copies, encodes the other pieces
//! with `EncoderProcess::start_segment`, joins, then verifies the joined file.
//! **Signals and state:** bounded FFmpeg runs with an optional cancel flag; the pieces, the list
//! and the output live in the caller's folder.
//! **Invariants:** piece `n`'s file is `copy{n:05}.mkv` or `encode{n:05}.mkv` in the folder; every
//! listed piece starts where the pieces before it end, by frame count at the constant rate, so no
//! container rounding accumulates; the joined video starts at the source video's offset; the
//! source is only read and the output never holds a subtitle or data stream.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use child_process::Run;

use super::args::{copied_piece_name, copy_args, remove_if_present};
use super::plan::Piece;
use crate::encode::process::{STDERR_TAIL_BYTES, tail};
use crate::encode::{path, strings};
use crate::{MediaError, Programs};

/// The concat list's name in the pieces' folder.
pub const JOIN_LIST: &str = "pieces.ffconcat";

/// The file of piece `number` in `folder`: the copy command's output for a copied piece, the
/// segment encode's output for a re-encoded one.
pub fn piece_file(folder: &Path, number: usize, piece: Piece) -> PathBuf {
    match piece {
        Piece::Copy { .. } => folder.join(copied_piece_name(number)),
        Piece::Encode { .. } => folder.join(format!("encode{number:05}.mkv")),
    }
}

/// Cut every copied piece of `pieces` from `source` into `folder` with one FFmpeg run, and
/// remove the cuts that re-encoded pieces replace. Nothing runs when no piece is copied.
pub fn copy_pieces(
    programs: &Programs,
    source: &Path,
    pieces: &[Piece],
    folder: &Path,
    timeout: Duration,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<(), MediaError> {
    if !pieces.iter().any(|piece| piece.is_copy()) {
        return Ok(());
    }
    let boundaries: Vec<u64> = pieces.iter().skip(1).map(|piece| piece.first()).collect();
    let frame_count = pieces.last().map_or(0, |piece| piece.last() + 1);
    run_ffmpeg(
        programs,
        copy_args(source, &boundaries, frame_count, folder),
        timeout,
        cancel,
    )?;
    for (number, piece) in pieces.iter().enumerate() {
        let cut = folder.join(copied_piece_name(number));
        if piece.is_copy() {
            if !cut.is_file() {
                return Err(MediaError::Parse(format!(
                    "the copy wrote no piece {number}"
                )));
            }
        } else {
            remove_if_present(&cut)?;
        }
    }
    Ok(())
}

/// The concat list of `pieces` in `folder`: each piece's file, named relative to the list, and
/// its duration at the constant `frame_rate`, rounded so that each piece starts at its first
/// frame's exact time rounded to the microsecond.
pub fn join_list(pieces: &[Piece], frame_rate: (u32, u32)) -> String {
    let (num, den) = (u128::from(frame_rate.0.max(1)), u128::from(frame_rate.1));
    let micros = |frame: u64| (u128::from(frame) * den * 1_000_000 + num / 2) / num;
    let mut list = String::from("ffconcat version 1.0\n");
    for (number, piece) in pieces.iter().enumerate() {
        let name = piece_file(Path::new(""), number, *piece);
        let duration = micros(piece.last() + 1) - micros(piece.first());
        list.push_str(&format!(
            "file '{}'\nduration {}.{:06}\n",
            name.to_string_lossy().replace('\'', "'\\''"),
            duration / 1_000_000,
            duration % 1_000_000
        ));
    }
    list
}

/// What a join reads and writes.
#[derive(Debug, Clone, PartialEq)]
pub struct JoinRequest {
    /// The folder holding every piece's file; the list is written there.
    pub folder: PathBuf,
    /// The source, only read: its audio, chapters and metadata are copied.
    pub source: PathBuf,
    /// When the source's first frame presents, origin-relative: `timeline[0].0`.
    pub video_offset_s: f64,
    pub frame_rate: (u32, u32),
    /// The Matroska file to write; an existing one is overwritten.
    pub output: PathBuf,
}

/// FFmpeg's arguments to join the pieces `list` names, offset to the source's video start,
/// with every audio stream, the chapters and the metadata of the source, into Matroska. The
/// concat demuxer converts nothing: the pieces carry their own headers.
pub fn join_args(list: &Path, request: &JoinRequest) -> Vec<String> {
    let mut args = strings(&[
        "-nostdin",
        "-hide_banner",
        "-v",
        "error",
        "-y",
        "-f",
        "concat",
        "-safe",
        "0",
        "-auto_convert",
        "0",
        "-itsoffset",
    ]);
    args.push(format!("{:.6}", request.video_offset_s));
    args.push("-i".into());
    args.push(path(list));
    args.push("-i".into());
    args.push(path(&request.source));
    args.extend(strings(&[
        "-map",
        "0:v:0",
        "-map",
        "1:a?",
        "-map_chapters",
        "1",
        "-map_metadata",
        "1",
        "-sn",
        "-dn",
        "-c",
        "copy",
        "-max_muxing_queue_size",
        "4096",
        "-f",
        "matroska",
    ]));
    args.push(path(&request.output));
    args
}

/// Write the concat list of `pieces` into the request's folder and join them into its output.
pub fn join_pieces(
    programs: &Programs,
    pieces: &[Piece],
    request: &JoinRequest,
    timeout: Duration,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<(), MediaError> {
    if request.output == request.source {
        return Err(MediaError::Parse(
            "the join would overwrite its source".into(),
        ));
    }
    let (num, den) = request.frame_rate;
    if pieces.is_empty() || num == 0 || den == 0 || !request.video_offset_s.is_finite() {
        return Err(MediaError::Parse("invalid join".into()));
    }
    let list = request.folder.join(JOIN_LIST);
    std::fs::write(&list, join_list(pieces, request.frame_rate))
        .map_err(|error| MediaError::Parse(format!("cannot write {}: {error}", list.display())))?;
    run_ffmpeg(programs, join_args(&list, request), timeout, cancel)
}

/// Run FFmpeg to completion: an exit error with the end of its stderr when it fails.
fn run_ffmpeg(
    programs: &Programs,
    args: Vec<String>,
    timeout: Duration,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<(), MediaError> {
    let mut run = Run::new(&programs.ffmpeg).args(args).timeout(timeout);
    if let Some(flag) = cancel {
        run = run.cancel_on(flag);
    }
    let output = run.output()?;
    if output.code != 0 {
        return Err(MediaError::Exit {
            program: programs.ffmpeg.clone(),
            code: output.code,
            stderr: tail(&output.stderr, STDERR_TAIL_BYTES),
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/join.rs"]
mod tests;
