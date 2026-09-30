//! Per-frame placement of moving writing.
//!
//! **Role:** find where the keyframe's writing sits in each frame of its span by zero-mean
//! normalized cross-correlation, searched around the position interpolated from the sampled
//! quads.
//! **Position:** used by background runs for writing whose sampled quads move; decodes the
//! span's search region once.
//! **Signals and state:** the keyframe template at five scales, full size and coarse; one
//! placement per frame of the span.
//! **Invariants:** a placement is an integer shift of the keyframe window centre and one of the
//! fixed scales; the search never leaves the given radius around the prediction; a frame
//! matched below the least correlation loses the whole occurrence.

use image::RgbImage;
use image::imageops::{self, FilterType};
use job_model::onscreen::{PixelRect, Point, TextFrame};

use super::correlation::{Integral, Plane, Template, grey};
use super::select::{bounding, clamp};
use super::{Outcome, RegionSource, check_size};
use crate::onscreen_text::TextResult;

/// Why writing that cannot be followed stays in the subtitle file.
pub(super) const UNFOLLOWED: &str = "The writing moves in a way that could not be followed";
/// Scales of the writing relative to the keyframe that the search tries.
pub(super) const SCALES: [f64; 5] = [0.9, 0.95, 1.0, 1.05, 1.1];
/// The index of scale 1.0 in [`SCALES`].
const UNIT_SCALE: usize = 2;
/// Least correlation of a followed frame.
const MIN_SCORE: f32 = 0.8;
/// Search radius: this share of the quad's shorter side plus [`RADIUS_PX`].
const RADIUS_SHARE: f64 = 0.5;
const RADIUS_PX: f64 = 16.0;
/// Longest template side of the coarse search, in pixels.
const COARSE_SIDE: u32 = 48;
/// Coarse candidates refined at full size.
const REFINED: usize = 2;

/// Where the writing sits in one frame relative to the keyframe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct Placement {
    pub dx: i32,
    pub dy: i32,
    /// Index into [`SCALES`].
    pub scale: usize,
}

impl Placement {
    /// The keyframe's own placement.
    pub(super) const KEY: Placement = Placement {
        dx: 0,
        dy: 0,
        scale: UNIT_SCALE,
    };

    pub(super) fn factor(self) -> f64 {
        SCALES[self.scale]
    }
}

/// Follow the writing through every frame of `span`, decoding its search region once.
pub(super) fn follow(
    source: &mut dyn RegionSource,
    tracker: &Tracker,
    span: (u64, u64),
) -> TextResult<Outcome<Vec<Placement>>> {
    let frame = source.frame_size();
    let timeline = source.timeline();
    let mut times = Vec::new();
    let mut union: Option<PixelRect> = None;
    for index in span.0..=span.1 {
        let Some(&(time_s, _)) = timeline.get(index as usize) else {
            return Err(format!("frame {index} is outside the timeline").into());
        };
        let Some(area) = tracker.search_area(time_s, frame) else {
            return Ok(Err(UNFOLLOWED));
        };
        times.push(time_s);
        union = Some(union.map_or(area, |u| bounding(u, area)));
    }
    let Some(union) = union else {
        return Ok(Err(UNFOLLOWED));
    };
    let mut placements = Vec::with_capacity(times.len());
    let mut lost = false;
    source.frames(union, span.0, span.1, &mut |index, image| {
        check_size(&image, union)?;
        let Some(&time_s) = times.get(index.wrapping_sub(span.0) as usize) else {
            return Err(format!("frame {index} is outside the requested span").into());
        };
        if !lost {
            match tracker.locate(&image, (union.x, union.y), time_s) {
                Some((placement, score)) if score >= MIN_SCORE => placements.push(placement),
                _ => lost = true,
            }
        }
        Ok(())
    })?;
    if lost {
        return Ok(Err(UNFOLLOWED));
    }
    if placements.len() != times.len() {
        return Err(format!(
            "the decoder returned {} of {} frames",
            placements.len(),
            times.len()
        )
        .into());
    }
    Ok(Ok(placements))
}

/// The keyframe writing and the sampled path it is searched along.
pub(super) struct Tracker {
    window: PixelRect,
    radius: i64,
    factor: usize,
    fine: Vec<Option<Template>>,
    coarse: Vec<Option<Template>>,
    path: Vec<(f64, Point)>,
    key_centre: Point,
}

impl Tracker {
    /// The template is `window` of the keyframe `plate` crop taken at `plate_rect`, searched
    /// within a radius set by the quad's `shorter_side`; `None` when the writing has no contrast
    /// to follow.
    pub(super) fn new(
        plate: &RgbImage,
        plate_rect: PixelRect,
        window: PixelRect,
        frames: &[TextFrame],
        key_time_s: f64,
        shorter_side: f64,
    ) -> Option<Tracker> {
        let crop = imageops::crop_imm(
            plate,
            window.x - plate_rect.x,
            window.y - plate_rect.y,
            window.width,
            window.height,
        )
        .to_image();
        let gray = grey(&crop);
        let (w, h) = gray.dimensions();
        let factor = w
            .max(h)
            .div_ceil(COARSE_SIDE)
            .min((w.min(h) / 4).max(1))
            .max(1) as usize;
        let mut fine = Vec::new();
        let mut coarse = Vec::new();
        for (i, &scale) in SCALES.iter().enumerate() {
            let scaled = if i == UNIT_SCALE {
                gray.clone()
            } else {
                let sw = ((f64::from(w) * scale).round() as u32).max(1);
                let sh = ((f64::from(h) * scale).round() as u32).max(1);
                imageops::resize(&gray, sw, sh, FilterType::Triangle)
            };
            let plane = Plane::from_gray(&scaled);
            coarse.push(
                (factor > 1)
                    .then(|| Template::new(&plane.shrink(factor)))
                    .flatten(),
            );
            fine.push(Template::new(&plane));
        }
        fine[UNIT_SCALE].as_ref()?;
        let path: Vec<(f64, Point)> = frames.iter().map(|f| (f.time_s, f.quad.center())).collect();
        let key_centre = interpolate(&path, key_time_s)?;
        Some(Tracker {
            window,
            radius: (RADIUS_SHARE * shorter_side + RADIUS_PX).ceil() as i64,
            factor,
            fine,
            coarse,
            path,
            key_centre,
        })
    }

    /// The shift the sampled quads predict at `time_s`, in pixels.
    fn predicted(&self, time_s: f64) -> (f64, f64) {
        interpolate(&self.path, time_s)
            .map(|p| (p.x - self.key_centre.x, p.y - self.key_centre.y))
            .unwrap_or((0.0, 0.0))
    }

    /// The pixels every candidate position at `time_s` may cover, clipped to the frame.
    fn search_area(&self, time_s: f64, frame: (u32, u32)) -> Option<PixelRect> {
        let (px, py) = self.predicted(time_s);
        let w = self.window;
        let growth = (SCALES[SCALES.len() - 1] - 1.0) / 2.0 * f64::from(w.width.max(w.height));
        let reach = self.radius + growth.ceil() as i64 + 2;
        let (dx, dy) = (px.round() as i64, py.round() as i64);
        clamp(
            i64::from(w.x) + dx - reach,
            i64::from(w.y) + dy - reach,
            i64::from(w.right()) + dx + reach,
            i64::from(w.bottom()) + dy + reach,
            frame,
        )
    }

    /// Centre of the keyframe window, the origin of every placement's shift and scale.
    fn centre(&self) -> Point {
        Point {
            x: f64::from(self.window.x) + f64::from(self.window.width) / 2.0,
            y: f64::from(self.window.y) + f64::from(self.window.height) / 2.0,
        }
    }

    /// The best placement in `image`, a crop whose top-left sits at `origin` in the frame, and
    /// its correlation; `None` when no candidate position fits inside the crop.
    fn locate(
        &self,
        image: &RgbImage,
        origin: (u32, u32),
        time_s: f64,
    ) -> Option<(Placement, f32)> {
        let (px, py) = self.predicted(time_s);
        let centre = self.centre();
        let (iw, ih) = (i64::from(image.width()), i64::from(image.height()));
        let ranges: Vec<Option<[i64; 4]>> = self
            .fine
            .iter()
            .map(|template| {
                let t = template.as_ref()?;
                let left = (centre.x + px - t.w as f64 / 2.0).round() as i64 - i64::from(origin.0);
                let top = (centre.y + py - t.h as f64 / 2.0).round() as i64 - i64::from(origin.1);
                let range = [
                    (left - self.radius).max(0),
                    (left + self.radius).min(iw - t.w as i64),
                    (top - self.radius).max(0),
                    (top + self.radius).min(ih - t.h as i64),
                ];
                (range[0] <= range[1] && range[2] <= range[3]).then_some(range)
            })
            .collect();
        let area = ranges
            .iter()
            .zip(&self.fine)
            .filter_map(|(r, t)| Some((r.as_ref()?, t.as_ref()?)))
            .fold(None::<[i64; 4]>, |acc, (r, t)| {
                let next = [r[0], r[1] + t.w as i64, r[2], r[3] + t.h as i64];
                Some(acc.map_or(next, |a| {
                    [
                        a[0].min(next[0]),
                        a[1].max(next[1]),
                        a[2].min(next[2]),
                        a[3].max(next[3]),
                    ]
                }))
            })?;
        let crop = imageops::crop_imm(
            image,
            area[0] as u32,
            area[2] as u32,
            (area[1] - area[0]) as u32,
            (area[3] - area[2]) as u32,
        )
        .to_image();
        let plane = Plane::from_gray(&grey(&crop));
        let integral = Integral::new(&plane);
        let local = |r: [i64; 4]| {
            [
                r[0] - area[0],
                r[1] - area[0],
                r[2] - area[2],
                r[3] - area[2],
            ]
        };
        let candidates: Vec<(usize, [i64; 4])> = if self.factor > 1 {
            self.coarse_candidates(&plane, &ranges, &local)
        } else {
            ranges
                .iter()
                .enumerate()
                .filter_map(|(i, r)| Some((i, local((*r)?))))
                .collect()
        };
        let mut best: Option<(usize, i64, i64, f32)> = None;
        for (scale, [x0, x1, y0, y1]) in candidates {
            let Some(t) = self.fine[scale].as_ref() else {
                continue;
            };
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let s = t.score(&plane, &integral, x as usize, y as usize);
                    if best.is_none_or(|b| s > b.3) {
                        best = Some((scale, x, y, s));
                    }
                }
            }
        }
        let (scale, x, y, score) = best?;
        let t = self.fine[scale].as_ref()?;
        let found_x = (x + area[0] + i64::from(origin.0)) as f64 + t.w as f64 / 2.0;
        let found_y = (y + area[2] + i64::from(origin.1)) as f64 + t.h as f64 / 2.0;
        Some((
            Placement {
                dx: (found_x - centre.x).round() as i32,
                dy: (found_y - centre.y).round() as i32,
                scale,
            },
            score,
        ))
    }

    /// Full-size position ranges around the best coarse matches.
    fn coarse_candidates(
        &self,
        plane: &Plane,
        ranges: &[Option<[i64; 4]>],
        local: &dyn Fn([i64; 4]) -> [i64; 4],
    ) -> Vec<(usize, [i64; 4])> {
        let f = self.factor as i64;
        let small = plane.shrink(self.factor);
        let integral = Integral::new(&small);
        let mut best: Vec<(usize, i64, i64, f32)> = Vec::new();
        for (scale, range) in ranges.iter().enumerate() {
            let (Some(range), Some(t)) = (range, self.coarse[scale].as_ref()) else {
                continue;
            };
            let [x0, x1, y0, y1] = local(*range);
            let mut top: Option<(i64, i64, f32)> = None;
            for y in y0.div_euclid(f)..=y1.div_euclid(f).min(small.h as i64 - t.h as i64) {
                for x in x0.div_euclid(f)..=x1.div_euclid(f).min(small.w as i64 - t.w as i64) {
                    let s = t.score(&small, &integral, x as usize, y as usize);
                    if top.is_none_or(|b| s > b.2) {
                        top = Some((x, y, s));
                    }
                }
            }
            if let Some((x, y, s)) = top {
                best.push((scale, x, y, s));
            }
        }
        best.sort_by(|a, b| b.3.total_cmp(&a.3));
        let slack = f / 2 + 1;
        best.into_iter()
            .take(REFINED)
            .filter_map(|(scale, x, y, _)| {
                let [x0, x1, y0, y1] = local((*ranges.get(scale)?)?);
                let r = [
                    (x * f - slack).max(x0),
                    (x * f + slack).min(x1),
                    (y * f - slack).max(y0),
                    (y * f + slack).min(y1),
                ];
                (r[0] <= r[1] && r[2] <= r[3]).then_some((scale, r))
            })
            .collect()
    }
}

/// The path's position at `time_s`, linear between samples and held beyond them.
fn interpolate(path: &[(f64, Point)], time_s: f64) -> Option<Point> {
    let after = path.partition_point(|(t, _)| *t <= time_s);
    match (after.checked_sub(1).map(|i| path[i]), path.get(after)) {
        (Some((t0, p0)), Some(&(t1, p1))) if t1 > t0 => {
            let u = (time_s - t0) / (t1 - t0);
            Some(Point {
                x: p0.x + (p1.x - p0.x) * u,
                y: p0.y + (p1.y - p0.y) * u,
            })
        }
        (Some((_, p)), _) => Some(p),
        (None, Some(&(_, p))) => Some(p),
        (None, None) => None,
    }
}
