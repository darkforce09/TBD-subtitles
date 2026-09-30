//! Occurrences, plates and replacements shared by the read-back check's tests.

#![allow(dead_code)]

use std::path::PathBuf;

use job_model::onscreen::{
    LetteringStyle, PixelRect, Plate, Point, Quad, ReplaceStatus, ReplacedText, TextFrame,
    TextOccurrence, TextPresentation, TextProvenance,
};

pub fn rect(x: u32, y: u32, width: u32, height: u32) -> PixelRect {
    PixelRect {
        x,
        y,
        width,
        height,
    }
}

pub fn quad(left: f64, top: f64, right: f64, bottom: f64) -> Quad {
    Quad([
        Point { x: left, y: top },
        Point { x: right, y: top },
        Point {
            x: right,
            y: bottom,
        },
        Point { x: left, y: bottom },
    ])
}

/// An occurrence tracked at `at` over its whole span, with `english`.
pub fn occurrence(id: &str, english: &str, at: Quad, ruby: Vec<Quad>) -> TextOccurrence {
    TextOccurrence {
        id: id.into(),
        start_s: 0.0,
        end_s: 10.0,
        japanese: "幹部塔".into(),
        english: Some(english.into()),
        confidence: 0.9,
        crops: Vec::new(),
        frames: vec![TextFrame {
            time_s: 0.0,
            end_s: 10.0,
            quad: at,
            confidence: 0.9,
            surface_rgb: None,
        }],
        provenance: TextProvenance::default(),
        presentation: TextPresentation::default(),
        warnings: Vec::new(),
        reviewed: false,
        rendered: None,
        source_fingerprint: None,
        keyframe: None,
        ruby,
    }
}

/// A plate over `rect` for frames `first..=last`, moved by `shift` from the keyframe.
pub fn plate(first: u64, last: u64, rect: PixelRect, shift: [f64; 2]) -> Plate {
    Plate {
        first_frame: first,
        last_frame: last,
        rect,
        shift,
        scale: 1.0,
        source: PathBuf::from("visual/masks/source.png"),
        mask: PathBuf::from("visual/masks/mask.png"),
        plate: None,
        patch: None,
    }
}

/// A baked replacement over its plates' frames whose writing is `line` pixels tall.
pub fn replaced(id: &str, plates: Vec<Plate>, line: f64) -> ReplacedText {
    ReplacedText {
        id: id.into(),
        first_frame: plates.first().map_or(0, |p| p.first_frame),
        last_frame: plates.last().map_or(0, |p| p.last_frame),
        status: ReplaceStatus::Baked,
        style: Some(LetteringStyle {
            fill_rgb: [255, 255, 255],
            outline_rgb: None,
            outline_px: 0.0,
            soft_outline: false,
            stroke_px: 3.0,
            line_height_px: line,
        }),
        container: None,
        plates,
        preview: None,
        lettering_quad: None,
    }
}
