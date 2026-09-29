//! Per-frame optical verification of visible writing.
//!
//! **Role:** confirm detected surface motion and reject unsafe replacement geometry.
//! **Position:** visual tracking step between OCR reading and translation/typesetting.
//! **Signals and state:** two grayscale frames, reusable LK pyramids and at most 64 points per
//! active occurrence; all image buffers are released before the next pipeline step.
//! **Invariants:** original PTS are unchanged; confidence alone never verifies geometry; any
//! unverified motion or occlusion forces a readable nearby translation with a review flag.

use std::path::Path;

use image::{GrayImage, RgbImage};
use inference::ocr::OcrError;
use job_model::onscreen::{Point, Quad, TextDocument, TextOccurrence, TextTreatment};
use job_model::outputs::VideoStream;
use media_io::{Programs, video_frames::FrameStream};
use optical_flow_lk::{TrackStatus, TrackerContext, good_features_to_track_grid};

use super::geometry;

struct TrackState {
    ordinal: usize,
    quad: Quad,
    points: Vec<(f32, f32)>,
    verified: bool,
}

pub fn track(
    document: &mut TextDocument,
    programs: &Programs,
    video: &Path,
    stream: &VideoStream,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<(), OcrError> {
    let mut schedule: Vec<_> = document
        .occurrences
        .iter()
        .enumerate()
        .flat_map(|(occurrence, text)| {
            text.frames
                .iter()
                .enumerate()
                .map(move |(frame, value)| (value.time_s, occurrence, frame))
        })
        .collect();
    if schedule
        .iter()
        .any(|(time, _, _)| !time.is_finite() || *time < 0.0)
    {
        return Err("Text tracking received invalid frame timestamps".into());
    }
    schedule.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    if schedule.is_empty() {
        progress(0, 0);
        return Ok(());
    }
    if document.width != stream.width || document.height != stream.height {
        return Err("Text tracking geometry does not match the source video dimensions".into());
    }
    let fps = stream
        .fps()
        .filter(|fps| fps.is_finite() && *fps > 0.0)
        .ok_or("Text tracking needs a valid frame rate")?;
    let mut frames = FrameStream::open(
        programs,
        video,
        (stream.width, stream.height),
        stream.start_time_s,
        fps,
    )?;
    let mut states: Vec<Option<TrackState>> =
        (0..document.occurrences.len()).map(|_| None).collect();
    let mut previous: Option<GrayImage> = None;
    let mut tracker = TrackerContext::new();
    let (mut cursor, mut ordinal) = (0usize, 0usize);
    let tolerance = 2.0 * f64::from(stream.height) / 1080.0;
    while let Some(frame) = frames.next_frame()? {
        let rgb = RgbImage::from_raw(stream.width, stream.height, frame.rgb)
            .ok_or("Text tracking received an invalid RGB frame")?;
        let current = image::imageops::grayscale(&rgb);
        drop(rgb);
        if cursor < schedule.len() && schedule[cursor].0 < frame.time_s - 1e-5 {
            return Err("Text observations do not match decoded presentation timestamps".into());
        }
        let matching = cursor < schedule.len() && (schedule[cursor].0 - frame.time_s).abs() <= 1e-5;
        let cut = previous
            .as_ref()
            .is_some_and(|prior| scene_cut(prior, &current));
        if matching && let Some(prior) = &previous {
            tracker.prepare(prior, &current, 3);
        }
        while cursor < schedule.len() && (schedule[cursor].0 - frame.time_s).abs() <= 1e-5 {
            let (_, occurrence, frame_index) = schedule[cursor];
            let text = &mut document.occurrences[occurrence];
            let detector_quad = text.frames[frame_index].quad;
            let mut verified = states[occurrence]
                .as_ref()
                .is_some_and(|state| state.verified);
            if let Some(state) = &states[occurrence] {
                let result = if state.ordinal + 1 != ordinal {
                    Err("text observations have a gap")
                } else if cut {
                    Err("a scene cut breaks the text track")
                } else {
                    verify(
                        &mut tracker,
                        state,
                        detector_quad,
                        tolerance,
                        (stream.width, stream.height),
                    )
                };
                match result {
                    Ok(quad) => {
                        text.frames[frame_index].quad = quad;
                        verified = true;
                    }
                    Err(reason) => flag(text, reason),
                }
            }
            // Fresh detection coordinates re-anchor every pair, preventing cumulative flow drift.
            let has_next = frame_index + 1 < text.frames.len();
            let points = if has_next {
                features(&current, detector_quad)
            } else {
                Vec::new()
            };
            if has_next && points.len() < 8 {
                flag(
                    text,
                    "too few surface features support perspective tracking",
                );
            }
            states[occurrence] = Some(TrackState {
                ordinal,
                quad: detector_quad,
                points,
                verified,
            });
            cursor += 1;
        }
        previous = Some(current);
        ordinal += 1;
        progress(ordinal, document.decoded_frames as usize);
    }
    frames.finish()?;
    if cursor != schedule.len() {
        return Err("Text observations extend beyond the decoded video".into());
    }
    for (text, state) in document.occurrences.iter_mut().zip(states) {
        if state.is_none_or(|state| !state.verified) {
            flag(text, "no consecutive frames verify the text surface");
        }
    }
    Ok(())
}

fn verify(
    tracker: &mut TrackerContext,
    state: &TrackState,
    detected: Quad,
    tolerance: f64,
    size: (u32, u32),
) -> Result<Quad, &'static str> {
    if state.points.len() < 8 {
        return Err("too few tracked features establish perspective");
    }
    let results = tracker.track_fb(&state.points, None, 15, 30, 1e-3, tolerance.min(0.7) as f32);
    let (mut source, mut target) = (Vec::new(), Vec::new());
    for (&(x, y), tracked) in state.points.iter().zip(results) {
        if tracked.status == TrackStatus::Tracked
            && tracked.error.is_finite()
            && tracked.error <= 20.0
        {
            source.push(Point {
                x: f64::from(x),
                y: f64::from(y),
            });
            target.push(Point {
                x: f64::from(tracked.pos.0),
                y: f64::from(tracked.pos.1),
            });
        }
    }
    if source.len() < 8 || source.len() * 4 < state.points.len() * 3 {
        return Err("occlusion or inconsistent forward/backward motion obscures the surface");
    }
    let fit = geometry::robust_fit(&source, &target, tolerance)
        .ok_or("surface correspondences do not establish a perspective transform")?;
    if fit.inliers.len() < 8
        || fit.inliers.len() * 4 < source.len() * 3
        || fit.rms_error > tolerance
        || fit.max_error > tolerance
    {
        return Err("perspective reprojection exceeds the pixel tolerance");
    }
    let (left, top, right, bottom) = state.quad.bounds();
    let coverage = fit.inliers.iter().fold(
        (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ),
        |(l, t, r, b), &index| {
            let p = source[index];
            (l.min(p.x), t.min(p.y), r.max(p.x), b.max(p.y))
        },
    );
    if coverage.2 - coverage.0 < (right - left) * 0.4
        || coverage.3 - coverage.1 < (bottom - top) * 0.4
    {
        return Err("tracked features cover too little of the text surface");
    }
    let quad = geometry::map_quad(&fit.homography, state.quad)
        .ok_or("the perspective transform folds or invalidates the text surface")?;
    let ratio = geometry::area(quad) / geometry::area(state.quad);
    if !(0.5..=2.0).contains(&ratio)
        || quad
            .0
            .iter()
            .any(|p| p.x < 0.0 || p.y < 0.0 || p.x > f64::from(size.0) || p.y > f64::from(size.1))
    {
        return Err("the tracked text moves outside its plausible visible surface");
    }
    let discrepancy = quad
        .0
        .iter()
        .zip(detected.0)
        .map(|(a, b)| geometry::distance(*a, b))
        .fold(0.0, f64::max);
    if discrepancy > tolerance {
        return Err("fresh detection and tracked placement disagree beyond the pixel tolerance");
    }
    Ok(quad)
}

fn features(image: &GrayImage, quad: Quad) -> Vec<(f32, f32)> {
    if !quad.valid() {
        return Vec::new();
    }
    let (left, top, right, bottom) = quad.bounds();
    let left = left.clamp(0.0, f64::from(image.width())).floor() as u32;
    let top = top.clamp(0.0, f64::from(image.height())).floor() as u32;
    let right = right.min(f64::from(image.width())).ceil() as u32;
    let bottom = bottom.min(f64::from(image.height())).ceil() as u32;
    if right <= left + 8 || bottom <= top + 8 {
        return Vec::new();
    }
    let crop = image::imageops::crop_imm(image, left, top, right - left, bottom - top).to_image();
    good_features_to_track_grid(&crop, 4, 2, 8, 0.05, 3, &[])
        .into_iter()
        .filter_map(|(x, y, _)| {
            let p = Point {
                x: f64::from(x + left),
                y: f64::from(y + top),
            };
            (geometry::contains(quad, p)
                && p.x >= 8.0
                && p.y >= 8.0
                && p.x < f64::from(image.width()) - 8.0
                && p.y < f64::from(image.height()) - 8.0)
                .then_some((p.x as f32, p.y as f32))
        })
        .take(64)
        .collect()
}

fn scene_cut(previous: &GrayImage, current: &GrayImage) -> bool {
    let (mut total, mut count) = (0u64, 0u64);
    for (a, b) in previous.as_raw().iter().zip(current.as_raw()).step_by(97) {
        total += u64::from(a.abs_diff(*b));
        count += 1;
    }
    count == 0 || total as f64 / count as f64 > 60.0
}

fn flag(text: &mut TextOccurrence, reason: &str) {
    text.presentation.treatment = TextTreatment::Nearby;
    let warning = format!("Tracking needs review: {reason}. English uses nearby placement.");
    if !text.warnings.contains(&warning) {
        text.warnings.push(warning);
    }
}
