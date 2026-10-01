//! One measured encode of the benchmark's clip, and the Markdown rows of its results.
//!
//! **Role:** decode the clip into raw frames, stream them through this process into one encoder
//! (as the localize stage does), time it, size its file and compare it with the source by PSNR;
//! format each result, or why there is none, as a table row.
//! **Position:** used by `encode_bench::run` for every segment and whole-video row.
//! **Signals and state:** two FFmpeg children per encode (the decoder and the encoder, joined
//! through this process) and one for the comparison; the encoded file in the bench's folder.
//! **Invariants:** the source is only read; a failed encode reports its error in its row; the
//! frame rate counts only whole frames that reached the encoder.

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use child_process::Run;
use media_io::Programs;
use media_io::video_frames::{PixelFormat, pipe};

use super::args::{Clip, decode_args, parse_psnr, psnr_args};

/// The longest one encode or comparison may run.
const DEADLINE: Duration = Duration::from_secs(4 * 3600);

/// What one encode of the clip gave.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measure {
    pub frames: u64,
    pub seconds: f64,
    pub bytes: u64,
    /// The mean PSNR against the source in dB, when the comparison ran.
    pub psnr_db: Option<f64>,
}

/// One table row: an encode, its preset, and its measure or why it has none.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub encoder: String,
    pub preset: String,
    pub result: Result<Measure, String>,
}

/// The source's clip and the raw frames streamed from it.
#[derive(Debug, Clone, Copy)]
pub struct Source<'a> {
    pub programs: &'a Programs,
    pub video: &'a Path,
    pub clip: Clip,
    pub format: PixelFormat,
    pub size: (u32, u32),
}

/// Stream the clip's frames into FFmpeg started with `encoder_args`, which writes `output`;
/// then compare `output` with the source.
pub fn measure(source: &Source, encoder_args: Vec<String>, output: &Path) -> Result<Measure> {
    let frame_bytes = source.format.frame_bytes(source.size)?;
    let started = Instant::now();
    let mut encoder = Run::new(&source.programs.ffmpeg)
        .args(encoder_args)
        .stdin_piped()
        .timeout(DEADLINE)
        .spawn()?;
    drop(encoder.take_stdout());
    let mut input = encoder.take_stdin().context("no encoder input pipe")?;
    let mut decoder = Run::new(&source.programs.ffmpeg)
        .args(decode_args(source.video, source.clip, source.format))
        .timeout(DEADLINE)
        .spawn()?;
    let mut frames = decoder.take_stdout().context("no decoder output pipe")?;
    // A pipe that cannot grow keeps its default size: slower, never wrong.
    let _ = pipe::enlarge(&frames);
    let copied = std::io::copy(&mut frames, &mut input);
    drop(input);
    drop(frames);
    let encoded = encoder.wait()?;
    let seconds = started.elapsed().as_secs_f64();
    let decoded = decoder.wait()?;
    anyhow::ensure!(
        encoded.code == 0,
        "the encoder exited with {}: {}",
        encoded.code,
        last_line(&encoded.stderr)
    );
    anyhow::ensure!(
        decoded.code == 0,
        "the decoder exited with {}: {}",
        decoded.code,
        last_line(&decoded.stderr)
    );
    let copied = copied.context("stream the frames to the encoder")?;
    let bytes = std::fs::metadata(output)
        .with_context(|| format!("read the size of {}", output.display()))?
        .len();
    Ok(Measure {
        frames: copied / frame_bytes as u64,
        seconds,
        bytes,
        psnr_db: psnr(source, output).ok().flatten(),
    })
}

/// The mean PSNR of `encoded` against the source's clip.
fn psnr(source: &Source, encoded: &Path) -> Result<Option<f64>> {
    let compared = Run::new(&source.programs.ffmpeg)
        .args(psnr_args(source.video, source.clip, encoded))
        .timeout(DEADLINE)
        .output()?;
    anyhow::ensure!(compared.code == 0, "{}", last_line(&compared.stderr));
    Ok(parse_psnr(&compared.stderr))
}

/// The last non-empty line of an FFmpeg log, where it says what failed.
fn last_line(log: &str) -> &str {
    log.lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("no message")
}

/// The header of every result table.
pub fn header() -> String {
    "| Encoder | Preset | fps | Size (MiB) | Mbit/s | PSNR (dB) |\n|---|---|---|---|---|---|\n"
        .to_string()
}

/// `row` as one Markdown line, its clip `duration_s` long.
pub fn render(row: &Row, duration_s: f64) -> String {
    let cells: Vec<String> = match &row.result {
        Ok(measure) => {
            let mib = measure.bytes as f64 / (1024.0 * 1024.0);
            let rate = |amount: f64, seconds: f64| (seconds > 0.0).then(|| amount / seconds);
            vec![
                optional(rate(measure.frames as f64, measure.seconds), 1),
                format!("{mib:.1}"),
                optional(rate(measure.bytes as f64 * 8.0 / 1e6, duration_s), 2),
                optional(measure.psnr_db, 2),
            ]
        }
        Err(reason) => vec![reason.clone(), String::new(), String::new(), String::new()],
    };
    let mut line = format!("| {} | {} |", escape(&row.encoder), escape(&row.preset));
    for cell in cells {
        line.push_str(&format!(" {} |", escape(&cell)));
    }
    line.push('\n');
    line
}

/// `value` with `decimals` decimals, or `n/a` when it was not measured.
fn optional(value: Option<f64>, decimals: usize) -> String {
    match value {
        Some(value) if value.is_finite() => format!("{value:.decimals$}"),
        Some(value) if value.is_infinite() => "inf".to_string(),
        _ => "n/a".to_string(),
    }
}

/// `cell` with its pipes escaped and its line breaks turned into spaces.
fn escape(cell: &str) -> String {
    cell.replace('|', "\\|").replace(['\r', '\n'], " ")
}

#[cfg(test)]
#[path = "tests/measure.rs"]
mod tests;
