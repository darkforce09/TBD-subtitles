//! Stroke-mask diagnostics on a finished job.
//!
//! **Role:** print the figures the stroke-mask step judges chosen occurrences by (rectangles,
//! sampled quads, each colour partition's coverage, cut share and largest piece, completeness
//! counts, the frames moving writing is followed through and the verdict), and with `--all`
//! rerun mask extraction over the whole reviewed document and compare every verdict with the one
//! the job recorded.
//! **Position:** the `mask-probe` command of the validation tool; runs the production
//! `replace::mask` code on the CPU, decoding regions of the job's source video with FFmpeg.
//! **Signals and state:** reads the job's `job.json`, `probe.json`, `visual/text_review.json` and
//! `visual/text_mask.json`; writes keyframe plates, masks and a rerun's files only under `--out`.
//! **Invariants:** the job's work directory and its source video are only read.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use image::{GrayImage, Rgb, RgbImage};
use job_model::onscreen::{
    Point, Quad, ReplaceStatus, ReplacementDocument, TextDocument, TextOccurrence,
};
use job_model::outputs::VideoStream;
use stages::onscreen_text::replace::inpaint;
use stages::onscreen_text::replace::mask::{self, Diagnosis, Following, Reading};
use stages::onscreen_text::replace::source::FfmpegRegions;

/// What to probe.
pub struct Request {
    pub work: PathBuf,
    pub ids: Vec<String>,
    /// Furigana quads given in place of each probed occurrence's own.
    pub ruby: Vec<Quad>,
    /// Where plates, masks and a rerun's files go; nothing is written without it.
    pub out: Option<PathBuf>,
    /// Rerun extraction over every occurrence and compare verdicts.
    pub all: bool,
}

/// Parse `left,top,right,bottom` in frame pixels as an axis-aligned quad.
pub fn parse_box(text: &str) -> Result<Quad> {
    let values: Vec<f64> = text
        .split(',')
        .map(|v| v.trim().parse::<f64>())
        .collect::<std::result::Result<_, _>>()
        .context("a box is four numbers: left,top,right,bottom")?;
    let [l, t, r, b] = values[..] else {
        anyhow::bail!("a box is four numbers: left,top,right,bottom");
    };
    anyhow::ensure!(r > l && b > t, "a box needs right > left and bottom > top");
    let p = |x, y| Point { x, y };
    Ok(Quad([p(l, t), p(r, t), p(r, b), p(l, b)]))
}

pub fn run(request: &Request) -> Result<()> {
    let reviewed: TextDocument =
        pipeline::work_dir::read_json(&request.work.join("visual/text_review.json"))?;
    let mut source = open_source(&request.work)?;
    for id in &request.ids {
        let mut occurrence = reviewed
            .occurrences
            .iter()
            .find(|o| &o.id == id)
            .with_context(|| format!("{id} is not in the reviewed text document"))?
            .clone();
        if !request.ruby.is_empty() {
            occurrence.ruby = request.ruby.clone();
        }
        match mask::diagnose(&occurrence, &mut source).map_err(|e| anyhow::anyhow!("{e}"))? {
            Ok(diagnosis) => {
                print_diagnosis(&occurrence, &diagnosis);
                if let Some(out) = &request.out {
                    save_images(out, id, &diagnosis)?;
                }
            }
            Err(reason) => println!("{id}: {reason}"),
        }
    }
    if request.all {
        let out = request
            .out
            .as_deref()
            .context("--all needs --out for the rerun's files")?;
        compare_all(&request.work, &reviewed, &mut source, out)?;
    }
    Ok(())
}

/// The job's source video with its probed stream.
fn open_source(work: &Path) -> Result<FfmpegRegions> {
    let job: serde_json::Value = pipeline::work_dir::read_json(&work.join("job.json"))?;
    let video = PathBuf::from(job["video"].as_str().context("job.json names no video")?);
    let probe: serde_json::Value = pipeline::work_dir::read_json(&work.join("probe.json"))?;
    let stream: VideoStream = serde_json::from_value(probe["probe"]["video"].clone())
        .context("probe.json has no video stream")?;
    let programs = media_io::Programs::beside_current_exe();
    FfmpegRegions::open(&programs, &video, &stream).map_err(|e| anyhow::anyhow!("{e}"))
}

fn rect(r: job_model::onscreen::PixelRect) -> String {
    format!("[{},{},{},{}]", r.x, r.y, r.right(), r.bottom())
}

fn quad_bounds(q: Quad) -> String {
    let (l, t, r, b) = q.bounds();
    format!("[{l:.0},{t:.0},{r:.0},{b:.0}]")
}

fn print_diagnosis(occurrence: &TextOccurrence, d: &Diagnosis) {
    println!(
        "{} {:?} -> {:?}",
        occurrence.id,
        occurrence.japanese,
        occurrence.english.as_deref().unwrap_or("")
    );
    let ruby: Vec<String> = occurrence.ruby.iter().map(|q| quad_bounds(*q)).collect();
    println!(
        "  keyframe {}  quad {}  ruby [{}]  window {}  analysis {}  plate {}  line height {:.1}",
        d.keyframe,
        quad_bounds(d.quad),
        ruby.join(" "),
        rect(d.window),
        rect(d.analysis),
        rect(d.plate),
        d.line_height
    );
    print_frames(occurrence, d.quad);
    for j in &d.trace.partitions {
        let reading = match j.reading {
            Reading::Lettering => "lettering",
            Reading::DenseOutlined => "dense",
            Reading::Panel => "panel",
            Reading::OutlinedOverRing => "outlined",
        };
        let separation = j
            .separation
            .map_or_else(|| "-".to_string(), |s| format!("{s:.2}"));
        println!(
            "  {reading:9} k={} separation {separation:>6}  coverage {:.3}  cut {:.3}  largest {:>4}px  {}{}",
            j.colours,
            j.coverage,
            j.cut_share,
            j.largest_piece,
            j.failure.unwrap_or("passes"),
            if j.chosen { "  <- chosen" } else { "" }
        );
    }
    if let Some(c) = d.trace.completeness {
        println!(
            "  completeness: joined {} px, stray {} px of mask {} px ({:.1} %)",
            c.joined,
            c.stray,
            c.mask_area,
            100.0 * c.stray as f64 / c.mask_area.max(1) as f64
        );
    }
    if let Some(style) = &d.style {
        println!(
            "  style: fill {:?} outline {:?} outline {:.1}px stroke {:.1}px soft {}",
            style.fill_rgb,
            style.outline_rgb,
            style.outline_px,
            style.stroke_px,
            style.soft_outline
        );
    }
    if let Some(q) = d.lettering {
        let ((ql, qt, qr, qb), (ll, lt, lr, lb)) = (d.quad.bounds(), q.bounds());
        let common = (qr.min(lr) - ql.max(ll)).max(0.0) * (qb.min(lb) - qt.max(lt)).max(0.0);
        println!(
            "  lettering refitted to {} ({:.0} % of the box, {:.0} % of it inside the box)",
            quad_bounds(q),
            100.0 * common / ((qr - ql) * (qb - qt)).max(1.0),
            100.0 * common / ((lr - ll) * (lb - lt)).max(1.0)
        );
    }
    match &d.following {
        None => {}
        Some(Err(reason)) => println!("  following: {reason}"),
        Some(Ok(following)) => print_following(following),
    }
    match d.verdict {
        Ok(()) => println!("  verdict: separated"),
        Err(reason) => println!("  verdict: {reason}"),
    }
}

/// The sampled quads and how far their corners stray from the keyframe quad `key`.
fn print_frames(occurrence: &TextOccurrence, key: Quad) {
    let offset = |q: Quad| {
        q.0.iter()
            .zip(key.0.iter())
            .map(|(a, b)| (a.x - b.x).abs().max((a.y - b.y).abs()))
            .fold(0.0, f64::max)
    };
    let largest = occurrence
        .frames
        .iter()
        .map(|f| offset(f.quad))
        .fold(0.0, f64::max);
    println!(
        "  {} sampled quads, corners at most {largest:.1}px from the keyframe quad",
        occurrence.frames.len()
    );
    for f in &occurrence.frames {
        println!(
            "    {:8.3}-{:8.3} s  {}  offset {:.1}px",
            f.time_s,
            f.end_s,
            quad_bounds(f.quad),
            offset(f.quad)
        );
    }
}

/// Each followed frame's match, one line per run of equal placements, and the path they make.
fn print_following(following: &Following) {
    let frames = &following.frames;
    let lost = frames.iter().filter(|f| !f.followed).count();
    let worst = frames.iter().map(|f| f.score).fold(f32::INFINITY, f32::min);
    println!(
        "  following: {} frames, {lost} below the least correlation, worst {worst:.3}",
        frames.len()
    );
    let mut start = 0;
    for end in 1..=frames.len() {
        if end < frames.len() && frames[end].placement == frames[start].placement {
            continue;
        }
        let run = &frames[start..end];
        let low = run.iter().map(|f| f.score).fold(f32::INFINITY, f32::min);
        let placement = run[0].placement.map_or_else(
            || "no position".to_string(),
            |(dx, dy, s)| format!("shift ({dx},{dy}) scale {s:.2}"),
        );
        println!(
            "    frames {}-{}  {placement}  lowest {low:.3}",
            run[0].frame,
            run[run.len() - 1].frame
        );
        start = end;
    }
    match following.path {
        Ok(true) => println!("  path: still"),
        Ok(false) => println!("  path: moving"),
        Err(reason) => println!("  path: {reason}"),
    }
}

/// The keyframe plate and, when separated, the plate with the mask tinted red.
fn save_images(out: &Path, id: &str, d: &Diagnosis) -> Result<()> {
    std::fs::create_dir_all(out)?;
    d.pixels.save(out.join(format!("{id}-plate.png")))?;
    if let Some(mask) = &d.mask {
        tinted(&d.pixels, mask).save(out.join(format!("{id}-mask.png")))?;
    }
    Ok(())
}

fn tinted(pixels: &RgbImage, mask: &GrayImage) -> RgbImage {
    RgbImage::from_fn(pixels.width(), pixels.height(), |x, y| {
        let p = pixels.get_pixel(x, y).0;
        if mask.get_pixel(x, y).0[0] > 0 {
            Rgb([255, p[1] / 3, p[2] / 3])
        } else {
            Rgb(p)
        }
    })
}

/// Rerun extraction into `out` and print every occurrence whose verdict differs from the job's.
fn compare_all(
    work: &Path,
    reviewed: &TextDocument,
    source: &mut FfmpegRegions,
    out: &Path,
) -> Result<()> {
    let recorded: ReplacementDocument =
        pipeline::work_dir::read_json(&work.join("visual/text_mask.json"))?;
    let before: HashMap<&str, &ReplaceStatus> = recorded
        .texts
        .iter()
        .map(|t| (t.id.as_str(), &t.status))
        .collect();
    let root = out.join("rerun");
    std::fs::create_dir_all(&root)?;
    let rerun =
        mask::extract(reviewed, source, &root, &|_, _| {}).map_err(|e| anyhow::anyhow!("{e}"))?;
    let label = |status: Option<&ReplaceStatus>| match status {
        Some(ReplaceStatus::Pending) => "separated".to_string(),
        Some(ReplaceStatus::Baked) => "baked".to_string(),
        Some(ReplaceStatus::Fallback(reason)) => reason.clone(),
        None => "absent".to_string(),
    };
    let separated = |s: &ReplaceStatus| !matches!(s, ReplaceStatus::Fallback(_));
    let old_masks: HashMap<&str, PathBuf> = recorded
        .texts
        .iter()
        .filter_map(|t| Some((t.id.as_str(), t.plates.first()?.mask.clone())))
        .collect();
    let (mut was, mut now) = (0, 0);
    for text in &rerun.texts {
        let old = before.get(text.id.as_str()).copied();
        was += usize::from(old.is_some_and(separated));
        now += usize::from(separated(&text.status));
        let overlap = match (old_masks.get(text.id.as_str()), text.plates.first()) {
            (Some(old), Some(new)) => {
                mask_overlap(&work.join(old), &root.join(&new.mask)).unwrap_or_default()
            }
            _ => String::new(),
        };
        println!(
            "{:18} before: {:55} now: {} {overlap}",
            text.id,
            label(old),
            label(Some(&text.status))
        );
    }
    println!("separated before {was}, now {now} of {}", rerun.texts.len());
    Ok(())
}

/// Print, for every filled occurrence of the job's inpainting document, the largest share of a
/// plate's erased pixels that still look like the lettering, and whether the residue check
/// would retry it.
pub fn residue(work: &Path) -> Result<()> {
    let filled: ReplacementDocument =
        pipeline::work_dir::read_json(&work.join("visual/text_inpaint.json"))?;
    let (mut checked, mut above) = (0, 0);
    for text in &filled.texts {
        let Some(style) = &text.style else { continue };
        let mut worst: Option<(usize, f64)> = None;
        for (index, plate) in text.plates.iter().enumerate() {
            let Some(path) = &plate.plate else { continue };
            let pixels = image::open(work.join(path))?.to_rgb8();
            let mask = image::open(work.join(&plate.mask))?.to_luma8();
            let share = inpaint::residue_share(&pixels, &mask, style);
            if worst.is_none_or(|(_, w)| share > w) {
                worst = Some((index, share));
            }
        }
        let Some((index, share)) = worst else {
            continue;
        };
        checked += 1;
        let retry = share > inpaint::MAX_RESIDUE_SHARE;
        above += usize::from(retry);
        println!(
            "{:18} fill {:?} line {:5.1}  worst plate {index:3}: {:5.2} %{}",
            text.id,
            style.fill_rgb,
            style.line_height_px,
            100.0 * share,
            if retry { "  <- retry" } else { "" }
        );
    }
    println!("{above} of {checked} filled occurrences above the residue limit");
    Ok(())
}

/// Intersection over union and area ratio of the job's first-plate mask and the rerun's, when
/// both cover the same rectangle.
fn mask_overlap(old: &Path, new: &Path) -> Option<String> {
    let old = image::open(old).ok()?.to_luma8();
    let new = image::open(new).ok()?.to_luma8();
    if old.dimensions() != new.dimensions() {
        return Some(format!(
            "(plate {}x{} was {}x{})",
            new.width(),
            new.height(),
            old.width(),
            old.height()
        ));
    }
    let (mut both, mut either, mut old_area, mut new_area) = (0usize, 0usize, 0usize, 0usize);
    for (a, b) in old.pixels().zip(new.pixels()) {
        let (a, b) = (a.0[0] > 0, b.0[0] > 0);
        both += usize::from(a && b);
        either += usize::from(a || b);
        old_area += usize::from(a);
        new_area += usize::from(b);
    }
    Some(format!(
        "(mask IoU {:.3}, area {:.2}x)",
        both as f64 / either.max(1) as f64,
        new_area as f64 / old_area.max(1) as f64
    ))
}
