//! Confirmation of every occurrence on the full-resolution picture of its keyframe, after the
//! scan, in a fixed order.
//!
//! **Role:** take each distinct keyframe in the order occurrences first need it, from memory when
//! the scan kept it and from a still decoded from the video otherwise, have the server detector
//! confirm the keyframes a chunk at a time, keep the occurrences it confirms with their crop and
//! keyframe image, and drop the rest as screening noise.
//! **Position:** the scan's last phase; reads the source's stills, the sessions' `confirm` and
//! the sibling `crops` module, and hands its PNGs to the writer thread.
//! **Signals and state:** one chunk of at most `CONFIRM_CHUNK` keyframes at a time, the next
//! chunk's stills decoding on a thread meanwhile, and the writer thread.
//! **Invariants:** the confirmation order depends only on the document, so it is the same on
//! every run; each distinct keyframe is confirmed once; every confirmed occurrence gets one crop
//! and one keyframe image, and an unconfirmed one leaves the document; the crop and keyframe
//! folders are emptied first, so a rerun never leaves stale files; every PNG is on disk when
//! confirmation returns.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use image::RgbImage;
use inference::ocr::pool::{ConfirmJob, PaddedFrame, TextScreening};
use job_model::onscreen::TextDocument;
use media_io::yuv::{Coefficients, YuvFrame};

use super::crops::confirm;
use super::screen::padded;
use super::source::{FrameSource, STILL_DECODERS};
use super::timing::{ScanStats, timed};
use super::writer::{PngJob, PngThread};
use crate::onscreen_text::TextResult;

/// Keyframes confirmed per call to the sessions, as many as the stills decoded at once.
const CONFIRM_CHUNK: usize = STILL_DECODERS;

/// Minimum confidence for an occurrence to qualify its keyframe for server confirmation.
/// Real on-screen signs in animation score >= 0.60, while persistent background noise scores
/// below 0.50.
pub(crate) const MIN_CONFIRM_CONFIDENCE: f64 = 0.50;

/// The scan's document with each occurrence's keyframe, `(frame position, frame index)`, and
/// the keyframes held in memory by frame index.
pub(crate) struct Closed {
    pub(crate) document: TextDocument,
    pub(crate) keyframes: Vec<Option<(usize, u64)>>,
    pub(crate) frames: BTreeMap<u64, Arc<YuvFrame>>,
    pub(crate) min_confirm_frames: usize,
}

/// Confirms every occurrence of `closed` on its keyframe and returns the document of the
/// confirmed ones. An occurrence without a keyframe, or whose keyframe shows no matching
/// full-resolution region, is screening noise and leaves the document.
pub(crate) fn confirm_keyframes(
    closed: Closed,
    colour: &Coefficients,
    source: &mut dyn FrameSource,
    pool: &mut dyn TextScreening,
    root: &Path,
    stats: &mut ScanStats,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> TextResult<TextDocument> {
    // A rerun's crops and stills replace the previous scan's; stale files never linger.
    for folder in ["visual/crops", "visual/keyframes"] {
        let folder = root.join(folder);
        std::fs::create_dir_all(&folder)?;
        for entry in std::fs::read_dir(&folder)? {
            let path = entry?.path();
            if path.is_file() {
                std::fs::remove_file(path)?;
            }
        }
    }
    let Closed {
        document,
        keyframes,
        frames,
        min_confirm_frames,
    } = closed;
    let mut order = Vec::new();
    let mut users: HashMap<u64, Vec<usize>> = HashMap::new();
    for (occurrence, keyframe) in keyframes.iter().enumerate() {
        if let Some((_, index)) = *keyframe {
            users.entry(index).or_default().push(occurrence);
        }
    }
    let mut seen = std::collections::HashSet::new();
    for (occurrence, keyframe) in keyframes.iter().enumerate() {
        if let Some((_, index)) = *keyframe {
            let item = &document.occurrences[occurrence];
            if item.frames.len() >= min_confirm_frames
                && item.confidence >= MIN_CONFIRM_CONFIDENCE
                && seen.insert(index)
            {
                order.push(index);
            }
        }
    }
    let decoded = usize::try_from(document.decoded_frames).unwrap_or(usize::MAX);
    let mut run = Confirmation {
        confirmed: vec![false; document.occurrences.len()],
        document,
        keyframes,
        users,
        frames,
        colour: *colour,
        writer: PngThread::start(root),
        done: decoded,
        total: decoded.saturating_add(order.len()),
    };
    let chunks: Vec<&[u64]> = order.chunks(CONFIRM_CHUNK).collect();
    let mut fetched = match chunks.first() {
        Some(chunk) => fetch(source, &run.fallbacks(chunk)),
        None => Ok((Vec::new(), Duration::ZERO)),
    };
    for (position, chunk) in chunks.iter().enumerate() {
        let (stills, spent) = fetched?;
        stats.stills_from_ffmpeg += spent;
        let next = chunks.get(position + 1).map(|next| run.fallbacks(next));
        let (confirmed, prefetched) = std::thread::scope(|scope| {
            let reader: &mut dyn FrameSource = &mut *source;
            let prefetch = next.map(|indices| scope.spawn(move || fetch(reader, &indices)));
            let confirmed = run.chunk(chunk, stills, pool, stats, progress);
            let prefetched = prefetch.map(|thread| {
                thread
                    .join()
                    .unwrap_or_else(|_| Err("A still decoding thread panicked".into()))
            });
            (confirmed, prefetched)
        });
        confirmed?;
        fetched = prefetched.unwrap_or_else(|| Ok((Vec::new(), Duration::ZERO)));
    }
    let Confirmation {
        mut document,
        confirmed,
        writer,
        ..
    } = run;
    writer.finish()?;
    let mut position = 0;
    document.occurrences.retain(|_| {
        let keep = confirmed[position];
        position += 1;
        keep
    });
    Ok(document)
}

/// The stills for `indices` from the video, and how long decoding them took.
fn fetch(source: &mut dyn FrameSource, indices: &[u64]) -> TextResult<(Vec<RgbImage>, Duration)> {
    if indices.is_empty() {
        return Ok((Vec::new(), Duration::ZERO));
    }
    let started = Instant::now();
    let stills = source.stills(indices)?;
    if stills.len() != indices.len() {
        return Err("The frame source returned a different number of stills".into());
    }
    Ok((stills, started.elapsed()))
}

/// The confirmation's state between chunks.
struct Confirmation {
    document: TextDocument,
    keyframes: Vec<Option<(usize, u64)>>,
    users: HashMap<u64, Vec<usize>>,
    frames: BTreeMap<u64, Arc<YuvFrame>>,
    confirmed: Vec<bool>,
    colour: Coefficients,
    writer: PngThread,
    done: usize,
    total: usize,
}

impl Confirmation {
    /// The keyframes of `chunk` not held in memory, in order.
    fn fallbacks(&self, chunk: &[u64]) -> Vec<u64> {
        chunk
            .iter()
            .copied()
            .filter(|index| !self.frames.contains_key(index))
            .collect()
    }

    /// Confirms one chunk of keyframes, `stills` holding the decoded ones in order.
    fn chunk(
        &mut self,
        chunk: &[u64],
        stills: Vec<RgbImage>,
        pool: &mut dyn TextScreening,
        stats: &mut ScanStats,
        progress: &(dyn Fn(usize, usize) + Sync),
    ) -> TextResult<()> {
        let (width, height) = (self.document.width, self.document.height);
        let mut decoded = stills.into_iter();
        let mut pictures = Vec::with_capacity(chunk.len());
        let mut jobs = Vec::with_capacity(chunk.len());
        for (seq, index) in chunk.iter().enumerate() {
            let (still, frame) = match self.frames.remove(index) {
                Some(held) => {
                    stats.keyframes_from_ram += 1;
                    timed(&mut stats.stills_from_ram, || {
                        from_memory(&held, &self.colour)
                    })?
                }
                None => {
                    stats.keyframes_from_ffmpeg += 1;
                    let still = decoded.next().ok_or("A keyframe still went missing")?;
                    let frame = pad(&still);
                    (still, frame)
                }
            };
            if still.dimensions() != (width, height) {
                return Err("A keyframe still does not match the source video dimensions".into());
            }
            pictures.push(still);
            jobs.push(ConfirmJob {
                seq: seq as u64,
                frame,
            });
        }
        let results = timed(&mut stats.confirm, || pool.confirm(jobs))?;
        if results.len() != chunk.len()
            || results
                .iter()
                .enumerate()
                .any(|(seq, result)| result.seq != seq as u64)
        {
            return Err("The server detector answered a different set of keyframes".into());
        }
        for ((&index, still), result) in chunk.iter().zip(pictures).zip(results) {
            self.apply(index, still, &result.regions)?;
            self.done += 1;
            progress(self.done, self.total);
        }
        Ok(())
    }

    /// Confirms every occurrence whose keyframe is `index` against the regions `found` on its
    /// still, and queues the crops and, when any is confirmed, the still.
    fn apply(
        &mut self,
        index: u64,
        still: RgbImage,
        found: &[(job_model::onscreen::Quad, f64)],
    ) -> TextResult<()> {
        let image = PathBuf::from(format!("visual/keyframes/frame-{index:08}.png"));
        let mut saved = false;
        for &occurrence in self.users.get(&index).into_iter().flatten() {
            let Some((position, _)) = self.keyframes[occurrence] else {
                continue;
            };
            let item = &mut self.document.occurrences[occurrence];
            if let Some((path, crop)) = confirm(item, position, &still, found, &image)? {
                self.confirmed[occurrence] = true;
                self.writer.send(PngJob::Crop { path, image: crop })?;
                saved = true;
            }
        }
        if saved {
            self.writer.send(PngJob::Keyframe { path: image, still })?;
        }
        Ok(())
    }
}

/// A held keyframe as an rgb24 still and its padded picture.
fn from_memory(frame: &YuvFrame, colour: &Coefficients) -> TextResult<(RgbImage, PaddedFrame)> {
    let picture = frame
        .picture()
        .ok_or("A held keyframe does not hold its picture")?;
    let padded = padded(&picture, colour);
    let bytes = padded.width as usize * padded.height as usize * 3;
    let still = RgbImage::from_raw(padded.width, padded.height, padded.rgb[..bytes].to_vec())
        .ok_or("A held keyframe has the wrong size")?;
    Ok((still, padded))
}

/// A decoded still padded below with black rows to a multiple of 32.
fn pad(still: &RgbImage) -> PaddedFrame {
    let (width, height) = still.dimensions();
    let padded_height = PaddedFrame::padded(height);
    let mut rgb = still.as_raw().clone();
    rgb.resize(width as usize * padded_height as usize * 3, 0);
    PaddedFrame {
        width,
        height,
        padded_height,
        rgb,
    }
}

#[cfg(test)]
#[path = "tests/confirm.rs"]
mod tests;
