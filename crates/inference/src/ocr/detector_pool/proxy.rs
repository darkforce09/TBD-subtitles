//! The proxy pass of screening: frames shrunk to the proxy width, and its regions merged back.
//!
//! **Role:** shrink each padded full-resolution frame to its proxy size by averaging the source
//! pixels each proxy pixel covers, scale the regions the proxy session finds back to frame
//! pixels, and add to the full-resolution regions only those the full-resolution ones do not
//! already cover.
//!
//! **Position:** called by the pool's screening threads around their proxy session's run.
//!
//! **Signals and state:** none; plain functions over frames and region lists.
//!
//! **Invariants:** a proxy frame is padded below with black rows like any screened frame; every
//! source pixel inside the frame counts toward exactly one proxy pixel; a scaled region is
//! clipped into the frame; full-resolution regions are never dropped or moved, and a proxy
//! region is kept only when the full-resolution regions cover less than `COVERED_SHARE` of its
//! bounding box.

use job_model::onscreen::{Point, Quad};
use rayon::prelude::*;

use super::regions::Regions;
use crate::ocr::pool::PaddedFrame;

/// The share of a proxy region's bounding box the full-resolution regions may cover before it
/// counts as found already.
pub const COVERED_SHARE: f64 = 0.5;

/// `frame` shrunk to `size` (no larger than the frame), each pixel the mean of the source pixels
/// it covers, padded below with black rows to a multiple of 32.
pub fn shrink(frame: &PaddedFrame, (width, height): (u32, u32)) -> PaddedFrame {
    let (source_width, source_height) = (frame.width as usize, frame.height as usize);
    let (width, height) = (width as usize, height as usize);
    let padded_height = PaddedFrame::padded(height as u32);
    let mut rgb = vec![0u8; width * padded_height as usize * 3];
    let span = |index: usize, total: usize, source: usize| {
        let start = index * source / total;
        let end = ((index + 1) * source / total).max(start + 1).min(source);
        start..end
    };
    rgb.par_chunks_mut(width * 3)
        .take(height)
        .enumerate()
        .for_each(|(y, row)| {
            let rows = span(y, height, source_height);
            for x in 0..width {
                let columns = span(x, width, source_width);
                let mut sum = [0u32; 3];
                for source_y in rows.clone() {
                    let line = source_y * source_width * 3;
                    for source_x in columns.clone() {
                        let pixel = line + source_x * 3;
                        for (channel, total) in sum.iter_mut().enumerate() {
                            *total += u32::from(frame.rgb[pixel + channel]);
                        }
                    }
                }
                let count = (rows.len() * columns.len()) as u32;
                for (channel, total) in sum.iter().enumerate() {
                    row[x * 3 + channel] = ((total + count / 2) / count) as u8;
                }
            }
        });
    PaddedFrame {
        width: width as u32,
        height: height as u32,
        padded_height,
        rgb,
    }
}

/// `regions`, found on a `from` proxy, in the pixels of a `to` frame, clipped into it.
pub fn scale_up(regions: Regions, from: (u32, u32), to: (u32, u32)) -> Regions {
    let sx = f64::from(to.0) / f64::from(from.0.max(1));
    let sy = f64::from(to.1) / f64::from(from.1.max(1));
    let (right, bottom) = (f64::from(to.0), f64::from(to.1));
    regions
        .into_iter()
        .map(|(quad, score)| {
            let corners = quad.0.map(|point| Point {
                x: (point.x * sx).clamp(0.0, right),
                y: (point.y * sy).clamp(0.0, bottom),
            });
            (Quad(corners), score)
        })
        .filter(|(quad, _)| quad.valid())
        .collect()
}

/// Add to `full` every region of `proxy` whose bounding box the regions of `full` cover less
/// than `COVERED_SHARE` of; `full` keeps its own regions and order.
pub fn merge(full: &mut Regions, proxy: Regions) {
    let found: Vec<_> = full.iter().map(|(quad, _)| quad.bounds()).collect();
    for (quad, score) in proxy {
        let bounds = quad.bounds();
        let area = (bounds.2 - bounds.0) * (bounds.3 - bounds.1);
        if area <= 0.0 {
            continue;
        }
        let covered: f64 = found.iter().map(|other| overlap(bounds, *other)).sum();
        if covered.min(area) / area < COVERED_SHARE {
            full.push((quad, score));
        }
    }
}

/// The area two `(left, top, right, bottom)` boxes share.
fn overlap(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> f64 {
    let width = a.2.min(b.2) - a.0.max(b.0);
    let height = a.3.min(b.3) - a.1.max(b.1);
    width.max(0.0) * height.max(0.0)
}

#[cfg(test)]
#[path = "tests/proxy.rs"]
mod tests;
