//! The variable lettering font: metrics at a chosen width and weight, and glyph paths.
//!
//! **Role:** measure and outline English lettering with Noto Sans at the `wdth` and `wght`
//! axis values the layout picks.
//! **Position:** font backend of `compose`; implements `layout::Metrics`.
//! **Signals and state:** the font file's bytes, parsed per weight into one face per width axis.
//! **Invariants:** the font file is read-only; glyph advances come from the varied face, so the
//! measured width equals the drawn width; missing glyphs are reported, never substituted.

use std::path::Path;

use tiny_skia::{Path as SkiaPath, PathBuilder};
use ttf_parser::{Face, OutlineBuilder, Tag};

use super::layout::{Metrics, Placement, WIDTH_AXES};
use crate::onscreen_text::TextResult;

/// The font file inside the `latin-fonts` model folder.
pub(crate) const FONT_FILE: &str = "NotoSans.ttf";

/// The lettering font's bytes.
pub(crate) struct LetteringFont {
    bytes: Vec<u8>,
}

/// The font at one weight, with a face per width axis.
pub(crate) struct FontMetrics<'a> {
    faces: Vec<(f64, Face<'a>)>,
    units: f64,
    cap_ratio: f64,
    descent_ratio: f64,
}

impl LetteringFont {
    /// Read and check the font in the `latin-fonts` folder.
    pub(crate) fn open(folder: &Path) -> TextResult<Self> {
        let path = folder.join(FONT_FILE);
        let bytes = std::fs::read(&path).map_err(|e| {
            format!(
                "Cannot read the lettering font {}: {e}; download the Latin fonts in Settings",
                path.display()
            )
        })?;
        Face::parse(&bytes, 0)
            .map_err(|e| format!("Invalid lettering font {}: {e}", path.display()))?;
        Ok(Self { bytes })
    }

    /// The first character of `text` the font cannot draw.
    pub(crate) fn missing_glyph(&self, text: &str) -> Option<char> {
        let face = Face::parse(&self.bytes, 0).ok()?;
        text.chars()
            .find(|c| !c.is_whitespace() && face.glyph_index(*c).is_none())
    }

    /// The font varied to `weight`, one face per width axis.
    pub(crate) fn metrics(&self, weight: f64) -> TextResult<FontMetrics<'_>> {
        let mut faces = Vec::with_capacity(WIDTH_AXES.len());
        for width_axis in WIDTH_AXES {
            let mut face =
                Face::parse(&self.bytes, 0).map_err(|e| format!("Invalid lettering font: {e}"))?;
            if face.is_variable() {
                face.set_variation(Tag::from_bytes(b"wdth"), width_axis as f32);
                face.set_variation(Tag::from_bytes(b"wght"), weight as f32);
            }
            faces.push((width_axis, face));
        }
        let face = &faces[0].1;
        let units = f64::from(face.units_per_em());
        if units <= 0.0 {
            return Err("The lettering font has no em size".into());
        }
        let cap_ratio = face
            .capital_height()
            .map(|height| f64::from(height) / units)
            .filter(|ratio| *ratio > 0.0)
            .unwrap_or(0.7);
        let descent_ratio = (-f64::from(face.descender()) / units).max(0.0);
        Ok(FontMetrics {
            faces,
            units,
            cap_ratio,
            descent_ratio,
        })
    }
}

impl FontMetrics<'_> {
    fn face(&self, width_axis: f64) -> &Face<'_> {
        self.faces
            .iter()
            .find(|(axis, _)| *axis == width_axis)
            .map(|(_, face)| face)
            .unwrap_or(&self.faces[0].1)
    }

    /// Glyph outlines of every line, in canvas pixels: area pixels times `scale`.
    pub(crate) fn path(
        &self,
        lines: &[Placement],
        size: f64,
        width_axis: f64,
        scale: f64,
    ) -> TextResult<Option<SkiaPath>> {
        let face = self.face(width_axis);
        let mut builder = Glyphs {
            path: PathBuilder::new(),
            origin: (0.0, 0.0),
            scale: 0.0,
        };
        let em = size / self.units;
        for line in lines {
            let mut pen = line.x;
            for c in line.text.chars() {
                let glyph = face
                    .glyph_index(c)
                    .ok_or_else(|| format!("The font has no glyph for “{c}”"))?;
                builder.origin = (pen * scale, line.baseline * scale);
                builder.scale = em * scale;
                if !c.is_whitespace() {
                    face.outline_glyph(glyph, &mut builder);
                }
                pen += f64::from(face.glyph_hor_advance(glyph).unwrap_or(0)) * em;
            }
        }
        Ok(builder.path.finish())
    }
}

impl Metrics for FontMetrics<'_> {
    fn line_width(&self, line: &str, size: f64, width_axis: f64) -> f64 {
        let face = self.face(width_axis);
        let units: f64 = line
            .chars()
            .map(|c| {
                face.glyph_index(c)
                    .and_then(|glyph| face.glyph_hor_advance(glyph))
                    .map(f64::from)
                    .unwrap_or(self.units * 0.6)
            })
            .sum();
        units * size / self.units
    }

    fn cap_ratio(&self) -> f64 {
        self.cap_ratio
    }

    fn descent_ratio(&self) -> f64 {
        self.descent_ratio
    }
}

/// Collects glyph contours into one path, flipping font units to canvas pixels.
struct Glyphs {
    path: PathBuilder,
    origin: (f64, f64),
    scale: f64,
}

impl Glyphs {
    fn point(&self, x: f32, y: f32) -> (f32, f32) {
        (
            (self.origin.0 + f64::from(x) * self.scale) as f32,
            (self.origin.1 - f64::from(y) * self.scale) as f32,
        )
    }
}

impl OutlineBuilder for Glyphs {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.path.move_to(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.path.line_to(x, y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x1, y1) = self.point(x1, y1);
        let (x, y) = self.point(x, y);
        self.path.quad_to(x1, y1, x, y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (x1, y1) = self.point(x1, y1);
        let (x2, y2) = self.point(x2, y2);
        let (x, y) = self.point(x, y);
        self.path.cubic_to(x1, y1, x2, y2, x, y);
    }

    fn close(&mut self) {
        self.path.close();
    }
}
