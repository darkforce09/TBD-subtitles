//! Portable vector glyph outlines for perspective-sensitive signs.
//!
//! **Role:** choose a readable local font and flatten its contours before perspective mapping.
//! **Position:** geometry helper for the on-screen ASS typesetter.
//! **Signals and state:** one owned font face; bounded contour vectors for one text occurrence.
//! **Invariants:** font files are read-only; unsupported glyphs fail explicitly; curves are
//! subdivided in their original coordinate space rather than transforming Bezier controls.

use fontdb::{Database, Family, Query};
use job_model::onscreen::{Point, Quad};
use ttf_parser::{Face, OutlineBuilder};

use super::{
    TextResult,
    geometry::{self, Homography},
};

pub struct Font {
    bytes: Vec<u8>,
    index: u32,
    pub name: String,
}

/// The largest text rectangle centred at the requested point while staying inside the surface.
pub fn anchored_quad(quad: Quad, anchor: Option<Point>) -> TextResult<Quad> {
    let Some(anchor) = anchor else {
        return Ok(quad);
    };
    let unit = Quad([
        Point { x: 0.0, y: 0.0 },
        Point { x: 1.0, y: 0.0 },
        Point { x: 1.0, y: 1.0 },
        Point { x: 0.0, y: 1.0 },
    ]);
    let transform = geometry::quad_to_quad(unit, quad).ok_or("Invalid replacement surface")?;
    let inverse = transform
        .try_inverse()
        .ok_or("The replacement surface is singular")?;
    let centre =
        geometry::project(&inverse, anchor).ok_or("Invalid requested lettering position")?;
    if !(0.0..1.0).contains(&centre.x) || !(0.0..1.0).contains(&centre.y) {
        return Err("The requested lettering position lies outside the safe surface".into());
    }
    let dx = centre.x.min(1.0 - centre.x);
    let dy = centre.y.min(1.0 - centre.y);
    let local = Quad([
        Point {
            x: centre.x - dx,
            y: centre.y - dy,
        },
        Point {
            x: centre.x + dx,
            y: centre.y - dy,
        },
        Point {
            x: centre.x + dx,
            y: centre.y + dy,
        },
        Point {
            x: centre.x - dx,
            y: centre.y + dy,
        },
    ]);
    geometry::map_quad(&transform, local)
        .ok_or_else(|| "The requested lettering position is too close to the surface edge".into())
}

impl Font {
    pub fn load() -> TextResult<Self> {
        let mut database = Database::new();
        if let Some(appdir) = std::env::var_os("APPDIR") {
            database.load_fonts_dir(std::path::Path::new(&appdir).join("usr/share/fonts"));
        }
        database.load_system_fonts();
        let families = [
            Family::Name("DejaVu Sans"),
            Family::Name("Liberation Sans"),
            Family::Name("Noto Sans"),
            Family::SansSerif,
        ];
        let id = database
            .query(&Query {
                families: &families,
                ..Query::default()
            })
            .ok_or("No readable sans-serif font is available for sign outlines")?;
        let name = database
            .face(id)
            .and_then(|face| face.families.first())
            .map(|(name, _)| name.clone())
            .ok_or("The selected font has no family name")?;
        let (bytes, index) = database
            .with_face_data(id, |bytes, index| (bytes.to_vec(), index))
            .ok_or("Cannot read the selected sign font")?;
        Face::parse(&bytes, index).map_err(|e| format!("Invalid sign font: {e:?}"))?;
        Ok(Self { bytes, index, name })
    }

    pub fn width(&self, text: &str, size: f64) -> f64 {
        let Ok(face) = Face::parse(&self.bytes, self.index) else {
            return text.chars().count() as f64 * size * 0.6;
        };
        let units = f64::from(face.units_per_em());
        text.chars()
            .map(|c| {
                face.glyph_index(c)
                    .and_then(|id| face.glyph_hor_advance(id))
                    .map(f64::from)
                    .unwrap_or(units * 0.6)
                    * size
                    / units
            })
            .sum()
    }

    pub fn missing_glyph(&self, text: &str) -> Option<char> {
        let face = Face::parse(&self.bytes, self.index).ok()?;
        text.chars()
            .find(|c| !c.is_whitespace() && face.glyph_index(*c).is_none())
    }

    pub fn drawing(
        &self,
        text: &str,
        quad: Quad,
        requested_size: Option<f64>,
    ) -> TextResult<String> {
        let face = Face::parse(&self.bytes, self.index)
            .map_err(|e| format!("Invalid sign font: {e:?}"))?;
        let width = (geometry::distance(quad.0[0], quad.0[1])
            + geometry::distance(quad.0[3], quad.0[2]))
            / 2.0;
        let height = (geometry::distance(quad.0[0], quad.0[3])
            + geometry::distance(quad.0[1], quad.0[2]))
            / 2.0;
        let source = Quad([
            Point { x: 0.0, y: 0.0 },
            Point { x: width, y: 0.0 },
            Point {
                x: width,
                y: height,
            },
            Point { x: 0.0, y: height },
        ]);
        let transform =
            geometry::quad_to_quad(source, quad).ok_or("Cannot map glyphs onto this surface")?;
        let lines: Vec<_> = text.lines().collect();
        let units = f64::from(face.units_per_em());
        let ascent = f64::from(face.ascender());
        let line_height = f64::from(face.ascender()) - f64::from(face.descender());
        let widths: Vec<_> = lines.iter().map(|line| self.width(line, units)).collect();
        let widest = widths.iter().copied().fold(0.0f64, f64::max);
        if widest <= 0.0 || lines.is_empty() || line_height <= 0.0 {
            return Err("The translated sign has no drawable text".into());
        }
        let fit_scale =
            (width * 0.86 / widest).min(height * 0.80 / (line_height * lines.len() as f64));
        let scale = requested_size
            .map(|size| size / units)
            .unwrap_or(fit_scale)
            .min(fit_scale);
        if !scale.is_finite() || scale <= 0.0 {
            return Err("Invalid glyph size".into());
        }
        if scale * units < 14.0 {
            return Err("English lettering is too small to read on this surface".into());
        }
        let top = (height - scale * line_height * lines.len() as f64) / 2.0;
        let mut outlines = Contours::new(transform);
        for (line_index, line) in lines.iter().enumerate() {
            let mut pen = (width - widths[line_index] * scale) / 2.0;
            let baseline = top + (ascent + line_index as f64 * line_height) * scale;
            for c in line.chars() {
                if ('\u{0300}'..='\u{036f}').contains(&c) {
                    return Err("Combining marks need shaped subtitle text".into());
                }
                let glyph = face
                    .glyph_index(c)
                    .ok_or_else(|| format!("Sign font has no glyph for {c:?}"))?;
                outlines.origin = Point {
                    x: pen,
                    y: baseline,
                };
                outlines.scale = scale;
                if !c.is_whitespace() && face.outline_glyph(glyph, &mut outlines).is_none() {
                    return Err(format!("Sign font has no outline for {c:?}").into());
                }
                pen += f64::from(face.glyph_hor_advance(glyph).unwrap_or(0)) * scale;
            }
        }
        outlines.finish()
    }
}

struct Contours {
    transform: Homography,
    origin: Point,
    scale: f64,
    last: Point,
    first: Point,
    output: String,
    vertices: usize,
    failed: bool,
}

impl Contours {
    fn new(transform: Homography) -> Self {
        Self {
            transform,
            origin: Point::default(),
            scale: 1.0,
            last: Point::default(),
            first: Point::default(),
            output: String::new(),
            vertices: 0,
            failed: false,
        }
    }
    fn point(&self, x: f32, y: f32) -> Point {
        Point {
            x: self.origin.x + f64::from(x) * self.scale,
            y: self.origin.y - f64::from(y) * self.scale,
        }
    }
    fn emit(&mut self, command: &str, point: Point) {
        self.vertices += 1;
        if self.vertices > 100_000 {
            self.failed = true;
            return;
        }
        if let Some(mapped) = geometry::project(&self.transform, point) {
            self.output.push_str(&format!(
                "{command} {} {} ",
                (mapped.x * 8.0).round() as i64,
                (mapped.y * 8.0).round() as i64
            ));
        } else {
            self.failed = true;
        }
    }
    fn curve(&mut self, p: [Point; 4], depth: u8) {
        if self.failed {
            return;
        }
        let projected: Option<Vec<_>> = p
            .iter()
            .map(|p| geometry::project(&self.transform, *p))
            .collect();
        let Some(q) = projected else {
            self.failed = true;
            return;
        };
        let flat = line_distance(q[1], q[0], q[3]).max(line_distance(q[2], q[0], q[3])) <= 0.15;
        if flat {
            self.emit("l", p[3]);
            return;
        }
        if depth >= 14 {
            self.failed = true;
            return;
        }
        let (a, b, c) = (
            midpoint(p[0], p[1]),
            midpoint(p[1], p[2]),
            midpoint(p[2], p[3]),
        );
        let (d, e) = (midpoint(a, b), midpoint(b, c));
        let f = midpoint(d, e);
        self.curve([p[0], a, d, f], depth + 1);
        self.curve([f, e, c, p[3]], depth + 1);
    }
    fn finish(self) -> TextResult<String> {
        if self.failed || self.output.is_empty() {
            Err("The glyph outline cannot be safely projected".into())
        } else {
            Ok(self.output)
        }
    }
}

impl OutlineBuilder for Contours {
    fn move_to(&mut self, x: f32, y: f32) {
        self.last = self.point(x, y);
        self.first = self.last;
        self.emit("m", self.last);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.last = self.point(x, y);
        self.emit("l", self.last);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let control = self.point(x1, y1);
        let end = self.point(x, y);
        let first = Point {
            x: self.last.x + (control.x - self.last.x) * 2.0 / 3.0,
            y: self.last.y + (control.y - self.last.y) * 2.0 / 3.0,
        };
        let second = Point {
            x: end.x + (control.x - end.x) * 2.0 / 3.0,
            y: end.y + (control.y - end.y) * 2.0 / 3.0,
        };
        self.curve([self.last, first, second, end], 0);
        self.last = end;
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let end = self.point(x, y);
        self.curve([self.last, self.point(x1, y1), self.point(x2, y2), end], 0);
        self.last = end;
    }
    fn close(&mut self) {
        self.emit("l", self.first);
        self.last = self.first;
    }
}

fn midpoint(a: Point, b: Point) -> Point {
    Point {
        x: (a.x + b.x) / 2.0,
        y: (a.y + b.y) / 2.0,
    }
}

fn line_distance(p: Point, a: Point, b: Point) -> f64 {
    let length = geometry::distance(a, b);
    if length < 1e-9 {
        return geometry::distance(p, a);
    }
    ((b.x - a.x) * (a.y - p.y) - (a.x - p.x) * (b.y - a.y)).abs() / length
}
