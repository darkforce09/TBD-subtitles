//! FFmpeg's `scdet` scan of a small scaled copy of the video: the times of the shot changes.
//!
//! **Role:** run the scan and read every scene change FFmpeg reports, with its score.
//! **Position:** called by the pipeline's shot scan step; the cue and on-screen text stages use
//! the cuts it returns.
//! **Signals and state:** one FFmpeg child process under a deadline; no state between calls.
//! **Invariants:** every change at or above the lowest reported score is kept with its score, in
//! time order; the stages choose the score that counts as a cut.

use std::path::Path;
use std::time::Duration;

use child_process::Run;
use job_model::outputs::{ShotChanges, ShotCut};

use crate::{MediaError, Programs};

/// The lowest scene score `scdet` reports. It is low on purpose: every change is kept with its
/// score, and the cue stage chooses the score that counts as a cut.
pub const REPORT_THRESHOLD: f64 = 10.0;

/// Scan `video` for cuts. `cuda_decode` decodes on the GPU (NVDEC), which the host has and the
/// development container does not; on a many-core CPU plain decoding is faster, because the
/// frames are scaled on the CPU either way.
pub fn scan(
    programs: &Programs,
    video: &Path,
    cuda_decode: bool,
    deadline: Duration,
) -> Result<ShotChanges, MediaError> {
    let mut run =
        Run::new(&programs.ffmpeg).args(["-nostdin", "-hide_banner", "-nostats", "-v", "info"]);
    if cuda_decode {
        run = run.args(["-hwaccel", "cuda"]);
    }
    let out = run
        .arg("-i")
        .arg(video)
        .args(["-an", "-sn", "-dn", "-vf"])
        .arg(format!("scale=480:-2,scdet=threshold={REPORT_THRESHOLD}"))
        .args(["-f", "null", "-"])
        .timeout(deadline)
        .output()?;
    if out.code != 0 {
        return Err(MediaError::Exit {
            program: programs.ffmpeg.clone(),
            code: out.code,
            stderr: out.stderr,
        });
    }
    Ok(parse(&out.stderr))
}

/// The `lavfi.scd.score` and `lavfi.scd.time` pairs in FFmpeg's log, ascending by time.
pub fn parse(log: &str) -> ShotChanges {
    let mut cuts: Vec<ShotCut> = log
        .lines()
        .filter_map(|line| {
            let score = value_after(line, "lavfi.scd.score:")?;
            let time_s = value_after(line, "lavfi.scd.time:")?;
            Some(ShotCut { time_s, score })
        })
        .collect();
    cuts.sort_by(|a, b| a.time_s.total_cmp(&b.time_s));
    cuts.dedup_by(|a, b| a.time_s == b.time_s);
    ShotChanges { cuts }
}

fn value_after(line: &str, key: &str) -> Option<f64> {
    let rest = line.split(key).nth(1)?;
    rest.split([',', ' '])
        .find(|t| !t.is_empty())?
        .trim()
        .parse()
        .ok()
}

#[cfg(test)]
#[path = "tests/shot_changes.rs"]
mod tests;
