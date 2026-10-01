use std::path::PathBuf;

use job_model::onscreen::{
    PixelRect, ReplaceStatus, TextKeyframe, TextPresentation, TextProvenance,
};

use super::*;

fn rect_quad(x: f64, y: f64, width: f64, height: f64) -> Quad {
    Quad([
        Point { x, y },
        Point { x: x + width, y },
        Point {
            x: x + width,
            y: y + height,
        },
        Point { x, y: y + height },
    ])
}

fn occurrence(start_s: f64, end_s: f64, frames: Vec<TextFrame>) -> TextOccurrence {
    TextOccurrence {
        id: "t1".into(),
        start_s,
        end_s,
        japanese: "店".into(),
        english: Some("SHOP".into()),
        confidence: 1.0,
        crops: Vec::new(),
        frames,
        provenance: TextProvenance::default(),
        presentation: TextPresentation::default(),
        warnings: Vec::new(),
        reviewed: true,
        rendered: None,
        source_fingerprint: None,
        keyframe: None,
        ruby: Vec::new(),
    }
}

fn member(first: u64, last: u64, quad: Quad) -> Member {
    Member {
        first_frame: first,
        last_frame: last,
        quad,
        line_height: 20.0,
    }
}

fn frame(time_s: f64, end_s: f64, x: f64) -> TextFrame {
    TextFrame {
        time_s,
        end_s,
        quad: rect_quad(x, 0.0, 10.0, 10.0),
        confidence: 1.0,
        surface_rgb: None,
    }
}

fn plate(first: u64, last: u64) -> Plate {
    Plate {
        first_frame: first,
        last_frame: last,
        rect: PixelRect {
            x: 100,
            y: 50,
            width: 200,
            height: 100,
        },
        shift: [0.0, 0.0],
        scale: 1.0,
        source: PathBuf::from("s.png"),
        mask: PathBuf::from("m.png"),
        plate: None,
        patch: None,
        shifted: Vec::new(),
    }
}

#[test]
fn nearby_writing_on_screen_together_forms_one_container() {
    let members = [
        // 0 and 1: 15 px apart vertically, both on screen at frames 10..20.
        member(0, 20, rect_quad(100.0, 100.0, 200.0, 30.0)),
        member(10, 40, rect_quad(100.0, 145.0, 200.0, 30.0)),
        // 2: far away.
        member(0, 40, rect_quad(900.0, 700.0, 100.0, 30.0)),
        // 3: next to 0 but never on screen with it or with 1.
        member(50, 60, rect_quad(100.0, 100.0, 200.0, 30.0)),
        // 4: chained to 1 (within one line height), not to 0 directly.
        member(30, 35, rect_quad(100.0, 190.0, 200.0, 30.0)),
    ];
    assert_eq!(group(&members), vec![vec![0, 1, 4], vec![2], vec![3]]);
}

#[test]
fn writing_more_than_a_line_height_apart_stays_separate() {
    let members = [
        member(0, 20, rect_quad(100.0, 100.0, 200.0, 30.0)),
        member(0, 20, rect_quad(100.0, 171.0, 200.0, 30.0)),
    ];
    assert_eq!(group(&members), vec![vec![0], vec![1]]);
    assert!(group(&[]).is_empty());
}

#[test]
fn the_keyframe_frame_contains_the_keyframe_time() {
    let mut occurrence = occurrence(
        0.0,
        3.0,
        vec![
            frame(0.0, 1.0, 0.0),
            frame(1.0, 2.0, 20.0),
            frame(2.0, 3.0, 40.0),
        ],
    );
    occurrence.keyframe = Some(TextKeyframe {
        time_s: 1.5,
        image: PathBuf::from("k.png"),
    });
    assert_eq!(keyframe_frame(&occurrence).unwrap().quad.0[0].x, 20.0);
    occurrence.keyframe = Some(TextKeyframe {
        time_s: 7.0,
        image: PathBuf::from("k.png"),
    });
    assert_eq!(keyframe_frame(&occurrence).unwrap().quad.0[0].x, 40.0);
    occurrence.keyframe = None;
    assert_eq!(keyframe_frame(&occurrence).unwrap().quad.0[0].x, 20.0);
    occurrence.frames.clear();
    assert!(keyframe_frame(&occurrence).is_none());
}

#[test]
fn the_keyframe_index_follows_its_time() {
    let mut occurrence = occurrence(10.0, 14.0, Vec::new());
    occurrence.keyframe = Some(TextKeyframe {
        time_s: 12.0,
        image: PathBuf::from("k.png"),
    });
    let text = ReplacedText {
        id: "t1".into(),
        first_frame: 100,
        last_frame: 199,
        status: ReplaceStatus::Pending,
        style: None,
        container: None,
        plates: vec![plate(100, 149), plate(150, 199)],
        preview: None,
        lettering_quad: None,
    };
    assert_eq!(keyframe_index(&text, &occurrence), 150);
    assert_eq!(plate_at(&text.plates, 150), Some(1));
    assert_eq!(plate_at(&text.plates, 120), Some(0));
    assert_eq!(plate_at(&text.plates[1..], 20), Some(0));
    assert_eq!(plate_at(&[], 20), None);
}

#[test]
fn a_plate_quad_is_scaled_about_the_centre_shifted_and_made_local() {
    let quad = rect_quad(140.0, 80.0, 100.0, 40.0);
    let mut placed = plate(0, 10);
    placed.scale = 1.5;
    placed.shift = [4.0, -2.0];
    let local = plate_quad(quad, &placed);
    // Centre (190, 100) becomes local (94, 48); half-size 75 by 30.
    assert_eq!(local.0[0], Point { x: 19.0, y: 18.0 });
    assert_eq!(local.0[2], Point { x: 169.0, y: 78.0 });
    let area = rectified(quad);
    assert!((area.width - 100.0).abs() < 1e-9 && (area.height - 40.0).abs() < 1e-9);
}
