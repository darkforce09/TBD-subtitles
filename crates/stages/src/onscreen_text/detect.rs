//! Frame observations and representative crops for visible writing.
//!
//! **Role:** scan every source frame and group consecutive matching regions.
//! **Position:** first visual stage, using FFmpeg and the local text detector.
//! **Signals and state:** one decoded frame, bounded detection cache and geometry metadata.
//! **Invariants:** duplicate frames reuse detections; no full-video image extraction occurs.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use super::{TextResult, geometry};
use image::{GrayImage, RgbImage, imageops};
use imageproc::geometric_transformations::{Interpolation, Projection, warp_into};
use inference::ocr::OcrDetector;
use job_model::onscreen::{
    Point, Quad, TextDocument, TextFrame, TextOccurrence, TextPresentation, TextProvenance,
};
use job_model::outputs::{ShotChanges, VideoStream};
use media_io::{Programs, video_frames::FrameStream};

const OBSERVATION_LIMIT: usize = 1_000_000;
const OCCURRENCE_LIMIT: usize = 100_000;

struct Active {
    occurrence: usize,
    quad: Quad,
    anchor: GrayImage,
}

struct Observation {
    quad: Quad,
    confidence: f64,
    signature: GrayImage,
    surface_rgb: Option<[u8; 3]>,
}

pub fn scan(
    programs: &Programs,
    video: &Path,
    stream: &VideoStream,
    cuts: &ShotChanges,
    root: &Path,
    detector: &mut OcrDetector,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> TextResult<TextDocument> {
    let mut frames = FrameStream::open(
        programs,
        video,
        (stream.width, stream.height),
        stream.start_time_s,
        stream.fps().unwrap_or(24.0),
    )?;
    let mut document = TextDocument {
        width: stream.width,
        height: stream.height,
        ..TextDocument::default()
    };
    std::fs::create_dir_all(root.join("visual/crops"))?;
    let mut last_hash = None;
    let mut detections = Vec::new();
    let mut active = Vec::new();
    let mut previous_time = 0.0;
    let mut observations = 0usize;
    while let Some(frame) = frames.next_frame()? {
        let image = RgbImage::from_raw(stream.width, stream.height, frame.rgb)
            .ok_or("invalid RGB frame size")?;
        let mut hasher = DefaultHasher::new();
        image.as_raw().hash(&mut hasher);
        let hash = hasher.finish();
        if last_hash != Some(hash) {
            detections = detector.detect(&image)?;
            last_hash = Some(hash);
        }
        if crosses_cut(cuts, previous_time, frame.time_s) {
            active.clear();
        }
        observations += detections.len();
        check_limits(observations, document.occurrences.len(), false)?;
        let current: Vec<_> = detections
            .iter()
            .map(|&(quad, confidence)| {
                let surface_rgb = simple_surface(&axis_crop(&image, quad));
                Observation {
                    quad,
                    confidence,
                    signature: signature(&crop(&image, quad)),
                    surface_rgb,
                }
            })
            .collect();
        let matches = associate(&active, &current);
        let mut previous: Vec<_> = active.into_iter().map(Some).collect();
        let mut next = Vec::with_capacity(current.len());
        for (observation, matched) in current.into_iter().zip(matches) {
            let mut state = if let Some(index) = matched {
                previous[index].take().expect("mutually unique association")
            } else {
                check_limits(observations, document.occurrences.len(), true)?;
                let occurrence =
                    start_occurrence(&mut document, frame.time_s, observation.confidence);
                crop(&image, observation.quad)
                    .save(root.join(&document.occurrences[occurrence].crops[0]))?;
                Active {
                    occurrence,
                    quad: observation.quad,
                    anchor: observation.signature.clone(),
                }
            };
            // The anchor crop is immutable: a later crop cannot erase the reading evidence.
            append_observation(
                &mut document.occurrences[state.occurrence],
                &observation,
                frame.time_s,
                frame.end_s,
            );
            state.quad = observation.quad;
            next.push(state);
        }
        active = next;
        document.decoded_frames += 1;
        previous_time = frame.time_s;
        progress(document.decoded_frames as usize, 0);
    }
    frames.finish()?;
    Ok(document)
}

fn check_limits(observations: usize, occurrences: usize, creating: bool) -> TextResult<()> {
    if observations > OBSERVATION_LIMIT || (creating && occurrences >= OCCURRENCE_LIMIT) {
        return Err("The visual scan reached its bounded observation limit. Split this unusually dense video into shorter jobs; no text is silently discarded.".into());
    }
    Ok(())
}

fn crosses_cut(cuts: &ShotChanges, previous: f64, current: f64) -> bool {
    cuts.cuts
        .iter()
        .any(|cut| cut.time_s > previous && cut.time_s <= current)
}

fn start_occurrence(document: &mut TextDocument, time: f64, confidence: f64) -> usize {
    let index = document.occurrences.len();
    let id = format!("text-{:06}", index + 1);
    document.occurrences.push(TextOccurrence {
        source_fingerprint: None,
        crops: vec![std::path::PathBuf::from(format!("visual/crops/{id}.png"))],
        id,
        start_s: time,
        end_s: time,
        japanese: String::new(),
        english: None,
        confidence,
        frames: Vec::new(),
        provenance: TextProvenance::default(),
        presentation: TextPresentation::default(),
        warnings: Vec::new(),
        reviewed: false,
        rendered: None,
    });
    index
}

fn append_observation(item: &mut TextOccurrence, observation: &Observation, start: f64, end: f64) {
    item.end_s = end;
    item.frames.push(TextFrame {
        time_s: start,
        end_s: end,
        quad: observation.quad,
        confidence: observation.confidence,
        surface_rgb: observation.surface_rgb,
    });
}

/// Two-way uniqueness prevents order-dependent swaps between overlapping text regions.
fn associate(active: &[Active], current: &[Observation]) -> Vec<Option<usize>> {
    let mut matches = Vec::with_capacity(current.len());
    let mut uses = vec![0usize; active.len()];
    for observation in current {
        let mut candidates = Vec::new();
        for (index, prior) in active.iter().enumerate() {
            if overlap(prior.quad, observation.quad) > 0.45
                && same_signature(&prior.anchor, &observation.signature)
            {
                candidates.push(index);
                uses[index] += 1;
            }
        }
        matches.push(if candidates.len() == 1 {
            candidates.first().copied()
        } else {
            None
        });
    }
    matches
        .into_iter()
        .map(|candidate| candidate.filter(|&index| uses[index] == 1))
        .collect()
}

fn signature(image: &RgbImage) -> GrayImage {
    let (w, h) = image.dimensions();
    let scale = (48.0 / f64::from(w.min(h)))
        .min(768.0 / f64::from(w.max(h)))
        .min(1.0);
    imageops::resize(
        &imageops::grayscale(image),
        (f64::from(w) * scale).round().max(1.0) as u32,
        (f64::from(h) * scale).round().max(1.0) as u32,
        imageops::FilterType::Triangle,
    )
}

/// Local limits preserve a changed glyph even when it occupies little of a long line.
fn same_signature(anchor: &GrayImage, current: &GrayImage) -> bool {
    if anchor == current {
        return true;
    }
    let ratio = f64::from(anchor.width()) / f64::from(current.width());
    let height_ratio = f64::from(anchor.height()) / f64::from(current.height());
    if !(0.9..=1.1).contains(&ratio)
        || !(0.9..=1.1).contains(&height_ratio)
        || !(0.95..=1.05).contains(&(ratio / height_ratio))
    {
        return false;
    }
    let resized;
    let current = if anchor.dimensions() == current.dimensions() {
        current
    } else {
        resized = imageops::resize(
            current,
            anchor.width(),
            anchor.height(),
            imageops::FilterType::Triangle,
        );
        &resized
    };
    let cost = |dx, dy| {
        (0..anchor.height())
            .step_by(4)
            .flat_map(|y| {
                (0..anchor.width())
                    .step_by(4)
                    .map(move |x| u64::from(delta(anchor, current, x, y, dx, dy)))
            })
            .sum::<u64>()
    };
    let (mut shift, mut best) = ((0, 0), cost(0, 0));
    for dy in -2..=2 {
        for dx in -2..=2 {
            let score = cost(dx, dy);
            if score < best {
                best = score;
                shift = (dx, dy);
            }
        }
    }
    let mut total = 0u64;
    for top in (0..anchor.height()).step_by(8) {
        for left in (0..anchor.width()).step_by(8) {
            let mut local = 0u64;
            let mut count = 0u64;
            for y in top..(top + 8).min(anchor.height()) {
                for x in left..(left + 8).min(anchor.width()) {
                    let difference = delta(anchor, current, x, y, shift.0, shift.1);
                    if difference > 180 {
                        return false;
                    }
                    local += u64::from(difference);
                    count += 1;
                }
            }
            if local > count * 6 {
                return false;
            }
            total += local;
        }
    }
    total <= u64::from(anchor.width()) * u64::from(anchor.height()) * 4
}

fn delta(a: &GrayImage, b: &GrayImage, x: u32, y: u32, dx: i64, dy: i64) -> u8 {
    let bx = (i64::from(x) + dx).clamp(0, i64::from(b.width()) - 1) as u32;
    let by = (i64::from(y) + dy).clamp(0, i64::from(b.height()) - 1) as u32;
    a.get_pixel(x, y).0[0].abs_diff(b.get_pixel(bx, by).0[0])
}

pub fn crop(image: &RgbImage, quad: Quad) -> RgbImage {
    let p = quad.0;
    let axis_aligned = (p[0].y - p[1].y).abs() < 0.1
        && (p[2].y - p[3].y).abs() < 0.1
        && (p[0].x - p[3].x).abs() < 0.1
        && (p[1].x - p[2].x).abs() < 0.1;
    if axis_aligned {
        return axis_crop(image, quad);
    }
    let width = geometry::distance(p[0], p[1])
        .max(geometry::distance(p[3], p[2]))
        .ceil() as u32;
    let height = geometry::distance(p[0], p[3])
        .max(geometry::distance(p[1], p[2]))
        .ceil() as u32;
    if width < 2 || height < 2 || u64::from(width) * u64::from(height) > 16_777_216 {
        return axis_crop(image, quad);
    }
    let target = Quad([
        Point { x: 0.0, y: 0.0 },
        Point {
            x: f64::from(width - 1),
            y: 0.0,
        },
        Point {
            x: f64::from(width - 1),
            y: f64::from(height - 1),
        },
        Point {
            x: 0.0,
            y: f64::from(height - 1),
        },
    ]);
    let Some(transform) = geometry::quad_to_quad(quad, target) else {
        return axis_crop(image, quad);
    };
    let Some(projection) =
        Projection::from_matrix(std::array::from_fn(|i| transform[(i / 3, i % 3)] as f32))
    else {
        return axis_crop(image, quad);
    };
    let mut output = RgbImage::new(width, height);
    warp_into(
        image,
        &projection,
        Interpolation::Bilinear,
        image::Rgb([255; 3]),
        &mut output,
    );
    output
}

fn axis_crop(image: &RgbImage, quad: Quad) -> RgbImage {
    let (left, top, right, bottom) = quad.bounds();
    let x = (left.floor().max(0.0) as u32).min(image.width().saturating_sub(1));
    let y = (top.floor().max(0.0) as u32).min(image.height().saturating_sub(1));
    let w = ((right.ceil() as u32).min(image.width()).saturating_sub(x)).max(1);
    let h = ((bottom.ceil() as u32).min(image.height()).saturating_sub(y)).max(1);
    imageops::crop_imm(image, x, y, w, h).to_image()
}

fn overlap(a: Quad, b: Quad) -> f64 {
    let (al, at, ar, ab) = a.bounds();
    let (bl, bt, br, bb) = b.bounds();
    let intersection = (ar.min(br) - al.max(bl)).max(0.0) * (ab.min(bb) - at.max(bt)).max(0.0);
    intersection / ((ar - al) * (ab - at) + (br - bl) * (bb - bt) - intersection).max(1.0)
}

/// A mask requires a nearly constant border and a dominant constant interior background.
fn simple_surface(image: &RgbImage) -> Option<[u8; 3]> {
    let color = image.get_pixel(0, 0).0;
    let similar =
        |pixel: &image::Rgb<u8>| pixel.0.iter().zip(color).all(|(a, b)| a.abs_diff(b) <= 5);
    let border = (0..image.width())
        .all(|x| similar(image.get_pixel(x, 0)) && similar(image.get_pixel(x, image.height() - 1)))
        && (0..image.height()).all(|y| {
            similar(image.get_pixel(0, y)) && similar(image.get_pixel(image.width() - 1, y))
        });
    if !border {
        return None;
    }
    let share = image.pixels().filter(|pixel| similar(pixel)).count() as f64
        / f64::from(image.width() * image.height());
    (share > 0.8).then_some(color)
}

#[cfg(test)]
#[path = "tests/detect.rs"]
mod tests;
