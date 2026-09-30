use tiny_skia::{PathBuilder, Rect};

use super::super::colours::Outline;
use super::*;

fn square() -> Path {
    let rect = Rect::from_ltrb(20.0, 20.0, 60.0, 60.0).unwrap();
    PathBuilder::from_rect(rect)
}

fn pixel(canvas: &Canvas, x: u32, y: u32) -> [u8; 4] {
    canvas.pixels[(y * canvas.width + x) as usize]
}

#[test]
fn the_canvas_is_supersampled_and_bounded() {
    let canvas = Canvas::blank(Area {
        width: 40.5,
        height: 20.0,
    })
    .unwrap();
    assert_eq!((canvas.width, canvas.height, canvas.scale), (81, 40, 2.0));
    let huge = Canvas::blank(Area {
        width: 8192.0,
        height: 100.0,
    })
    .unwrap();
    assert_eq!(huge.width, 4096);
    assert!((huge.scale - 0.5).abs() < 1e-9);
    assert!(
        Canvas::blank(Area {
            width: 0.0,
            height: 0.0
        })
        .is_err()
    );
}

#[test]
fn fill_sits_over_the_outline() {
    let mut canvas = Canvas::blank(Area {
        width: 40.0,
        height: 40.0,
    })
    .unwrap();
    let path = square();
    let colours = Colours {
        fill: [255, 0, 0],
        outline: Some(Outline {
            rgb: [0, 0, 255],
            width: 2.0,
            soft: false,
        }),
    };
    paint(&mut canvas, &path, &colours).unwrap();
    assert_eq!(pixel(&canvas, 40, 40), [255, 0, 0, 255]);
    // Two area pixels (four canvas pixels) outside the edge: outline only.
    assert_eq!(pixel(&canvas, 17, 40), [0, 0, 255, 255]);
    assert_eq!(pixel(&canvas, 5, 5), [0, 0, 0, 0]);
}

#[test]
fn a_soft_outline_fades_out() {
    let colours = |soft| Colours {
        fill: [255, 255, 255],
        outline: Some(Outline {
            rgb: [0, 0, 0],
            width: 4.0,
            soft,
        }),
    };
    let area = Area {
        width: 40.0,
        height: 40.0,
    };
    let mut hard = Canvas::blank(area).unwrap();
    let mut soft = Canvas::blank(area).unwrap();
    let path = square();
    paint(&mut hard, &path, &colours(false)).unwrap();
    paint(&mut soft, &path, &colours(true)).unwrap();
    // Just inside the hard outline's end, the soft one is partly transparent; just outside, it
    // still shows.
    assert_eq!(pixel(&hard, 13, 40)[3], 255);
    assert!(pixel(&soft, 13, 40)[3] < 255);
    assert_eq!(pixel(&hard, 11, 40)[3], 0);
    assert!(pixel(&soft, 11, 40)[3] > 0);
    // The fill itself stays opaque.
    assert_eq!(pixel(&soft, 40, 40), [255, 255, 255, 255]);
}

#[test]
fn lettering_without_outline_is_fill_only() {
    let mut canvas = Canvas::blank(Area {
        width: 40.0,
        height: 40.0,
    })
    .unwrap();
    let path = square();
    let colours = Colours {
        fill: [10, 200, 30],
        outline: None,
    };
    paint(&mut canvas, &path, &colours).unwrap();
    assert_eq!(pixel(&canvas, 30, 30), [10, 200, 30, 255]);
    assert_eq!(pixel(&canvas, 18, 30), [0, 0, 0, 0]);
}

#[test]
fn box_blur_keeps_flat_areas_and_spreads_edges() {
    let mut flat = vec![100u8; 25];
    box_blur(&mut flat, 5, 5, 2);
    assert!(flat.iter().all(|&v| v == 100));
    let mut dot = vec![0u8; 25];
    dot[12] = 225;
    box_blur(&mut dot, 5, 5, 1);
    assert_eq!(dot[12], 25);
    assert_eq!(dot[6], 25);
    assert_eq!(dot[0], 0);
}
