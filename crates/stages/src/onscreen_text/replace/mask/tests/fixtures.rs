//! Synthetic frames for mask extraction tests: glyph-like strokes, optional outlines and
//! scripted backgrounds, served through a fake region source.

use image::{GrayImage, Luma, RgbImage};
use imageproc::distance_transform::Norm;
use imageproc::morphology::dilate;
use job_model::onscreen::{
    PixelRect, Point, Quad, TextFrame, TextKeyframe, TextOccurrence, TextPresentation,
    TextProvenance,
};

use crate::onscreen_text::TextResult;
use crate::onscreen_text::replace::RegionSource;

pub(crate) const FPS: f64 = 24.0;
pub(crate) const FILL: [u8; 3] = [245, 240, 230];
pub(crate) const OUTLINE: [u8; 3] = [20, 20, 30];

/// Every pixel of every frame comes from a function of frame index and position.
pub(crate) struct Scripted {
    pub timeline: Vec<(f64, f64)>,
    pub size: (u32, u32),
    pub paint: Box<dyn Fn(u64, u32, u32) -> [u8; 3]>,
    pub requests: Vec<(PixelRect, u64, u64)>,
}

impl Scripted {
    pub(crate) fn new(
        frames: usize,
        size: (u32, u32),
        paint: impl Fn(u64, u32, u32) -> [u8; 3] + 'static,
    ) -> Scripted {
        Scripted {
            timeline: (0..frames)
                .map(|i| (i as f64 / FPS, (i + 1) as f64 / FPS))
                .collect(),
            size,
            paint: Box::new(paint),
            requests: Vec::new(),
        }
    }
}

impl RegionSource for Scripted {
    fn timeline(&self) -> &[(f64, f64)] {
        &self.timeline
    }

    fn frame_size(&self) -> (u32, u32) {
        self.size
    }

    fn frames(
        &mut self,
        rect: PixelRect,
        first: u64,
        last: u64,
        visit: &mut dyn FnMut(u64, RgbImage) -> TextResult<()>,
    ) -> TextResult<()> {
        assert!(
            rect.inside(self.size.0, self.size.1),
            "request outside the frame"
        );
        assert!(
            last < self.timeline.len() as u64,
            "request past the timeline"
        );
        self.requests.push((rect, first, last));
        for index in first..=last {
            let image = RgbImage::from_fn(rect.width, rect.height, |x, y| {
                image::Rgb((self.paint)(index, rect.x + x, rect.y + y))
            });
            visit(index, image)?;
        }
        Ok(())
    }
}

/// What a glyph pixel is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ink {
    Fill,
    Outline,
}

/// Blocky glyph strokes laid out in a row, with an optional outline.
#[derive(Clone)]
pub(crate) struct Glyphs {
    /// Fill rectangles as (left, top, right, bottom), exclusive right and bottom.
    pub strokes: Vec<(i64, i64, i64, i64)>,
    pub outline: i64,
}

impl Glyphs {
    /// Four blocky characters with 5-pixel strokes starting at `(left, top)`, each 26 × 30.
    pub(crate) fn row(left: i64, top: i64, outline: i64) -> Glyphs {
        let mut strokes = Vec::new();
        for c in 0..4 {
            let x = left + c * 38;
            strokes.push((x, top, x + 26, top + 5));
            strokes.push((x + 10, top, x + 15, top + 30));
            match c % 2 {
                0 => strokes.push((x, top + 25, x + 26, top + 30)),
                _ => strokes.push((x, top + 12, x + 26, top + 17)),
            }
        }
        Glyphs { strokes, outline }
    }

    /// `count` small furigana-like characters with 2-pixel strokes starting at `(left, top)`,
    /// each 10 × 12 and 14 apart.
    pub(crate) fn small_row(left: i64, top: i64, count: i64, outline: i64) -> Glyphs {
        let mut strokes = Vec::new();
        for c in 0..count {
            let x = left + c * 14;
            strokes.push((x, top, x + 10, top + 2));
            strokes.push((x + 4, top, x + 6, top + 12));
            strokes.push((x, top + 10, x + 10, top + 12));
        }
        Glyphs { strokes, outline }
    }

    /// The glyphs moved by `(dx, dy)`.
    pub(crate) fn shifted(&self, dx: i64, dy: i64) -> Glyphs {
        Glyphs {
            strokes: self
                .strokes
                .iter()
                .map(|&(l, t, r, b)| (l + dx, t + dy, r + dx, b + dy))
                .collect(),
            outline: self.outline,
        }
    }

    pub(crate) fn at(&self, x: u32, y: u32) -> Option<Ink> {
        let (x, y) = (i64::from(x), i64::from(y));
        let o = self.outline;
        if self
            .strokes
            .iter()
            .any(|&(l, t, r, b)| x >= l && x < r && y >= t && y < b)
        {
            Some(Ink::Fill)
        } else if o > 0
            && self
                .strokes
                .iter()
                .any(|&(l, t, r, b)| x >= l - o && x < r + o && y >= t - o && y < b + o)
        {
            Some(Ink::Outline)
        } else {
            None
        }
    }

    /// The bounds of the ink as (left, top, right, bottom).
    pub(crate) fn bounds(&self) -> (i64, i64, i64, i64) {
        let o = self.outline;
        self.strokes.iter().fold(
            (i64::MAX, i64::MAX, i64::MIN, i64::MIN),
            |(l, t, r, b), s| {
                (
                    l.min(s.0 - o),
                    t.min(s.1 - o),
                    r.max(s.2 + o),
                    b.max(s.3 + o),
                )
            },
        )
    }

    /// A detector-like quad: the ink bounds padded by `pad` pixels.
    pub(crate) fn quad(&self, pad: f64) -> Quad {
        let (l, t, r, b) = self.bounds();
        rect_quad(
            l as f64 - pad,
            t as f64 - pad,
            r as f64 + pad,
            b as f64 + pad,
        )
    }

    /// The truth mask over `rect`: every ink pixel dilated by `radius`.
    pub(crate) fn truth(&self, rect: PixelRect, radius: u8) -> GrayImage {
        let ink = GrayImage::from_fn(rect.width, rect.height, |x, y| {
            Luma([if self.at(rect.x + x, rect.y + y).is_some() {
                255
            } else {
                0
            }])
        });
        dilate(&ink, Norm::L2, radius)
    }

    /// Paint the glyphs over `background`.
    pub(crate) fn paint(&self, x: u32, y: u32, background: [u8; 3]) -> [u8; 3] {
        match self.at(x, y) {
            Some(Ink::Fill) => FILL,
            Some(Ink::Outline) => OUTLINE,
            None => background,
        }
    }
}

/// The keyframe plate around `quad` in a 640 × 360 frame painted by `paint`, its rectangle, and
/// the analysis window.
pub(crate) fn plate_of(
    quad: Quad,
    paint: &dyn Fn(u32, u32) -> [u8; 3],
) -> (RgbImage, PixelRect, PixelRect) {
    use super::select;
    let plate_rect = select::around(quad, select::context_margin(quad), 640, 360).expect("plate");
    let window =
        select::around(quad, f64::from(select::ANALYSIS_MARGIN), 640, 360).expect("window");
    let image = RgbImage::from_fn(plate_rect.width, plate_rect.height, |x, y| {
        image::Rgb(paint(plate_rect.x + x, plate_rect.y + y))
    });
    (image, plate_rect, window)
}

/// Segment one line without furigana inside a detector box: `window` is the analysis window.
pub(crate) fn segment_line(
    image: &RgbImage,
    plate: PixelRect,
    window: PixelRect,
    quad: Quad,
    line_height: f64,
) -> Result<super::segment::Segmentation, &'static str> {
    let areas = super::select::Areas {
        quad,
        ruby: Vec::new(),
        window,
        analysis: window,
        plate,
        line_height,
        refit: false,
    };
    super::segment::segment(image, &areas, &mut super::Trace::default())
}

/// Segment one line inside a detector box with furigana boxes `ruby` folded in: the analysis
/// window takes in the furigana as extraction's does.
pub(crate) fn segment_with_ruby(
    image: &RgbImage,
    plate: PixelRect,
    quad: Quad,
    ruby: &[Quad],
    line_height: f64,
    trace: &mut super::Trace,
) -> Result<super::segment::Segmentation, &'static str> {
    use super::select;
    let bounds = ruby.iter().fold(quad.bounds(), |(l, t, r, b), q| {
        let (ql, qt, qr, qb) = q.bounds();
        (l.min(ql), t.min(qt), r.max(qr), b.max(qb))
    });
    let union = rect_quad(bounds.0, bounds.1, bounds.2, bounds.3);
    let margin = f64::from(select::ANALYSIS_MARGIN);
    let areas = select::Areas {
        quad,
        ruby: ruby.to_vec(),
        window: select::around(quad, margin, 640, 360).expect("window"),
        analysis: select::around(union, margin, 640, 360).expect("analysis"),
        plate,
        line_height,
        refit: false,
    };
    super::segment::segment(image, &areas, trace)
}

/// The first of `layers` drawn at a pixel over `background`: fill before outline.
pub(crate) fn layered(layers: &[&Glyphs], x: u32, y: u32, background: [u8; 3]) -> [u8; 3] {
    let hits: Vec<Ink> = layers.iter().filter_map(|g| g.at(x, y)).collect();
    if hits.contains(&Ink::Fill) {
        FILL
    } else if hits.contains(&Ink::Outline) {
        OUTLINE
    } else {
        background
    }
}

/// A busy cartoon picture: diagonal bands of teal, beige and red with slight grain, split by
/// dark line art.
pub(crate) fn busy(x: u32, y: u32) -> [u8; 3] {
    const COLOURS: [[u8; 3]; 3] = [[60, 150, 150], [220, 200, 160], [180, 60, 60]];
    let along = x + 2 * y;
    if along % 37 < 2 {
        return [30, 30, 35];
    }
    let base = COLOURS[(along / 37 % 3) as usize];
    let grain = (hash(u64::from(x), u64::from(y), 3) % 9) as i16 - 4;
    base.map(|c| (i16::from(c) + grain).clamp(0, 255) as u8)
}

/// A translucent grey name card over a picture, closed by a black rule along its bottom.
pub(crate) struct Card {
    /// (left, top, right, bottom), exclusive right and bottom.
    pub rect: (u32, u32, u32, u32),
}

impl Card {
    const GREY: [u8; 3] = [90, 90, 95];
    const OPACITY: f64 = 0.55;
    const RULE: u32 = 4;

    /// The card's pixel over `under`, or `under` itself outside the card.
    pub(crate) fn over(&self, x: u32, y: u32, under: [u8; 3]) -> [u8; 3] {
        let (l, t, r, b) = self.rect;
        if x < l || x >= r || y < t || y >= b {
            return under;
        }
        if y + Card::RULE >= b {
            return [10, 10, 10];
        }
        std::array::from_fn(|c| {
            (f64::from(under[c]) * (1.0 - Card::OPACITY) + f64::from(Card::GREY[c]) * Card::OPACITY)
                .round() as u8
        })
    }
}

pub(crate) fn rect_quad(l: f64, t: f64, r: f64, b: f64) -> Quad {
    Quad([
        Point { x: l, y: t },
        Point { x: r, y: t },
        Point { x: r, y: b },
        Point { x: l, y: b },
    ])
}

/// A deterministic hash of three numbers, for noise.
pub(crate) fn hash(a: u64, b: u64, c: u64) -> u64 {
    let mut z = a
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(b.wrapping_mul(0xBF58_476D_1CE4_E5B9))
        .wrapping_add(c.wrapping_mul(0x94D0_49BB_1331_11EB));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The backgrounds the tests draw writing over.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Background {
    Flat,
    Gradient,
    Checker,
    Noise,
}

impl Background {
    pub(crate) fn at(self, x: u32, y: u32) -> [u8; 3] {
        match self {
            Background::Flat => [60, 90, 150],
            Background::Gradient => {
                let u = f64::from(x) / 640.0;
                [
                    (30.0 + 70.0 * u) as u8,
                    (60.0 + 80.0 * u) as u8,
                    (140.0 + 50.0 * u) as u8,
                ]
            }
            Background::Checker => {
                if (x / 8 + y / 8).is_multiple_of(2) {
                    [100, 110, 125]
                } else {
                    [135, 145, 160]
                }
            }
            Background::Noise => {
                let n = (hash(u64::from(x), u64::from(y), 7) % 41) as i32 - 20;
                [(110 + n) as u8, (120 + n) as u8, (135 + n) as u8]
            }
        }
    }
}

/// A translated, displayable occurrence with one observed frame per `(time_s, end_s, quad)`.
pub(crate) fn occurrence(id: &str, frames: &[(f64, f64, Quad)], key_time_s: f64) -> TextOccurrence {
    TextOccurrence {
        ruby: Vec::new(),
        id: id.into(),
        start_s: frames.first().map_or(0.0, |f| f.0),
        end_s: frames.last().map_or(0.0, |f| f.1),
        japanese: "文字".into(),
        english: Some("Letters".into()),
        confidence: 0.95,
        crops: Vec::new(),
        frames: frames
            .iter()
            .map(|&(time_s, end_s, quad)| TextFrame {
                time_s,
                end_s,
                quad,
                confidence: 0.9,
                surface_rgb: None,
            })
            .collect(),
        provenance: TextProvenance::default(),
        presentation: TextPresentation::default(),
        warnings: Vec::new(),
        reviewed: false,
        rendered: None,
        source_fingerprint: None,
        keyframe: Some(TextKeyframe {
            time_s: key_time_s,
            image: "visual/keyframes/k.png".into(),
        }),
    }
}

/// A static occurrence covering frames `first..=last` of the fixture timeline.
pub(crate) fn still_occurrence(id: &str, quad: Quad, first: u64, last: u64) -> TextOccurrence {
    let start = first as f64 / FPS;
    let end = (last + 1) as f64 / FPS;
    occurrence(id, &[(start, end, quad)], (start + end) / 2.0)
}

/// Intersection over union of two masks of equal size.
pub(crate) fn iou(a: &GrayImage, b: &GrayImage) -> f64 {
    let (mut both, mut either) = (0usize, 0usize);
    for (p, q) in a.pixels().zip(b.pixels()) {
        let (p, q) = (p.0[0] > 0, q.0[0] > 0);
        both += usize::from(p && q);
        either += usize::from(p || q);
    }
    both as f64 / either.max(1) as f64
}

/// A job directory under the system's temporary folder, deleted when dropped.
pub(crate) struct JobDir(std::path::PathBuf);

impl std::ops::Deref for JobDir {
    type Target = std::path::Path;

    fn deref(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for JobDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A fresh job directory named after the test.
pub(crate) fn job_dir(name: &str) -> JobDir {
    let dir = std::env::temp_dir().join(format!("tbd-mask-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the job directory");
    JobDir(dir)
}
