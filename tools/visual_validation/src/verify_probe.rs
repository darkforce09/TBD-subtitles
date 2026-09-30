//! Read-back check diagnostics on a finished job.
//!
//! **Role:** run the production read-back check over a job's composed replacements and print,
//! per sampled frame, the region read, every line found with its box score, reading and
//! confidence, whether it counted, the similarity to the English and the verdict; then each
//! occurrence's result and the similarity distribution the thresholds are tuned from.
//! **Position:** the `verify-probe` command of the validation tool; runs
//! `replace::verify::verify` with PP-OCRv5 on CUDA (on the host), decoding regions of the job's
//! source video with FFmpeg.
//! **Signals and state:** reads the job's `job.json`, `probe.json`, `visual/text_review.json`,
//! `visual/text_compose.json` and the patch files; writes the finished regions it read only under
//! `--out`.
//! **Invariants:** the job's work directory and its source video are only read.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use image::RgbImage;
use job_model::onscreen::{ReplaceStatus, ReplacementDocument, TextDocument};
use job_model::outputs::VideoStream;
use stages::localize::{colour::Conversion, frame_format};
use stages::onscreen_text::replace::source::FfmpegRegions;
use stages::onscreen_text::replace::verify::{self, LocalOcr, Request, Sample};

/// Check the job in `work`, only the occurrences `ids` when any are given; save each region read
/// under `out`.
pub fn run(work: &Path, ids: &[String], out: Option<&Path>) -> Result<()> {
    let job: serde_json::Value = pipeline::work_dir::read_json(&work.join("job.json"))?;
    let video = PathBuf::from(job["video"].as_str().context("job.json names no video")?);
    let models = match job["models_dir"].as_str() {
        Some(dir) => PathBuf::from(dir),
        None => inference::model_store::models_dir()?,
    };
    let probe: serde_json::Value = pipeline::work_dir::read_json(&work.join("probe.json"))?;
    let stream: VideoStream = serde_json::from_value(probe["probe"]["video"].clone())
        .context("probe.json has no video stream")?;
    let composed: ReplacementDocument =
        pipeline::work_dir::read_json(&work.join("visual/text_compose.json"))?;
    let text: TextDocument = pipeline::work_dir::read_json(&work.join("visual/text_review.json"))?;
    if let Some(out) = out {
        std::fs::create_dir_all(out)?;
    }
    let mut ocr = LocalOcr::open(&models).map_err(|e| anyhow::anyhow!("{e}"))?;
    let programs = media_io::Programs::beside_current_exe();
    let mut source =
        FfmpegRegions::open(&programs, &video, &stream).map_err(|e| anyhow::anyhow!("{e}"))?;
    let request = Request {
        composed: &composed,
        text: &text,
        root: work,
        conversion: Conversion::of(&stream, frame_format(&stream)),
        only: (!ids.is_empty()).then_some(ids),
    };
    let started = std::time::Instant::now();
    let mut saved: Result<()> = Ok(());
    let mut observe = |sample: &Sample, picture: &RgbImage| {
        print_sample(sample);
        if let (Some(out), Ok(())) = (out, &saved) {
            let file = out.join(format!("{}-{:06}.png", sample.id, sample.reading.frame));
            saved = picture.save(&file).context("save a finished region");
        }
    };
    let verified = verify::verify(&request, &mut source, &mut ocr, &mut observe, &|_, _| {})
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    saved?;
    println!();
    let mut similarities = Vec::new();
    for check in &verified.checks {
        let status = verified
            .document
            .texts
            .iter()
            .find(|t| t.id == check.id)
            .map(|t| &t.status);
        let english = text
            .occurrences
            .iter()
            .find(|o| o.id == check.id)
            .and_then(|o| o.english.as_deref())
            .unwrap_or("");
        let verdict = match status {
            Some(ReplaceStatus::Baked) => "approved".to_string(),
            Some(ReplaceStatus::Fallback(reason)) => format!("fallback: {reason}"),
            _ => "pending".to_string(),
        };
        let least = check
            .readings
            .iter()
            .map(|r| r.similarity)
            .fold(f64::INFINITY, f64::min);
        similarities.extend(check.readings.iter().map(|r| r.similarity));
        println!(
            "{} {english:?}: {} samples, least similarity {least:.3}, {verdict}",
            check.id,
            check.readings.len()
        );
    }
    let approved = verified
        .checks
        .iter()
        .filter(|check| {
            verified
                .document
                .texts
                .iter()
                .any(|t| t.id == check.id && t.status == ReplaceStatus::Baked)
        })
        .count();
    println!(
        "\n{approved} of {} checked approved in {:.1} s",
        verified.checks.len(),
        started.elapsed().as_secs_f64()
    );
    print_distribution(&similarities);
    Ok(())
}

fn print_sample(sample: &Sample) {
    let r = sample.area.region;
    let (l, t, rr, b) = sample.area.lettering;
    let reading = &sample.reading;
    println!(
        "{} frame {} region [{},{},{},{}] lettering [{l:.0},{t:.0},{rr:.0},{b:.0}] line {:.0} ×{:.2} similarity {:.3} {}",
        sample.id,
        reading.frame,
        r.x,
        r.y,
        r.right(),
        r.bottom(),
        sample.area.line_px,
        sample.area.scale,
        reading.similarity,
        if reading.passed { "pass" } else { "FAIL" }
    );
    for line in &sample.lines {
        let (left, top, right, bottom) = line.quad.bounds();
        // E: read as the English; J: where the writing was; -: neither.
        let place = match (line.read.over_lettering, line.read.over_writing) {
            (true, true) => "EJ",
            (true, false) => "E ",
            (false, true) => " J",
            (false, false) => "- ",
        };
        println!(
            "    {place} box [{left:.0},{top:.0},{right:.0},{bottom:.0}] score {:.2} read {:?} conf {:.2}",
            line.score, line.read.text, line.read.confidence
        );
    }
    if !reading.japanese_found.is_empty() {
        println!("    Japanese: {:?}", reading.japanese_found);
    }
}

/// Every sample's similarity in tenths, and the lowest.
fn print_distribution(similarities: &[f64]) {
    let mut buckets = [0usize; 11];
    for s in similarities {
        buckets[(s.clamp(0.0, 1.0) * 10.0).floor() as usize] += 1;
    }
    println!("similarity per sample:");
    for (tenth, count) in buckets.iter().enumerate().filter(|(_, c)| **c > 0) {
        println!("  {:.1}+ {count}", tenth as f64 / 10.0);
    }
}
