use job_model::onscreen::{PixelRect, Point, Quad, TextFrame};

use super::{around, context_margin, frame_at, is_static, keyframe_quad, line_height, span};

const FPS: f64 = 24.0;

fn timeline(frames: usize) -> Vec<(f64, f64)> {
    (0..frames)
        .map(|i| (i as f64 / FPS, (i + 1) as f64 / FPS))
        .collect()
}

fn quad(l: f64, t: f64, r: f64, b: f64) -> Quad {
    Quad([
        Point { x: l, y: t },
        Point { x: r, y: t },
        Point { x: r, y: b },
        Point { x: l, y: b },
    ])
}

fn frame(time_s: f64, end_s: f64, quad: Quad) -> TextFrame {
    TextFrame {
        time_s,
        end_s,
        quad,
        confidence: 1.0,
        surface_rgb: None,
    }
}

#[test]
fn spans_cover_the_frames_starting_inside_the_interval() {
    let t = timeline(10);
    let frame_start = |i: f64| i / FPS;
    assert_eq!(span(&t, 0.0, frame_start(10.0)), Some((0, 9)));
    assert_eq!(
        span(&t, frame_start(1.0) - 0.0005, frame_start(5.0)),
        Some((1, 4))
    );
    assert_eq!(
        span(&t, frame_start(1.0) + 0.0005, frame_start(5.0) + 0.0005),
        Some((1, 4))
    );
    assert_eq!(
        span(&t, frame_start(1.0) + 0.002, frame_start(5.0) + 0.002),
        Some((2, 5))
    );
    assert_eq!(
        span(&t, 0.3, 5.0),
        Some((8, 9)),
        "an end past the video keeps the last frame"
    );
    assert_eq!(
        span(&t, frame_start(1.0) + 0.01, frame_start(1.0) + 0.02),
        None
    );
    assert_eq!(span(&t, 1.0, 2.0), None, "after the last frame");
    assert_eq!(span(&[], 0.0, 1.0), None);
}

#[test]
fn a_time_maps_to_the_frame_showing_it() {
    let t = timeline(10);
    assert_eq!(frame_at(&t, 0.0), Some(0));
    assert_eq!(frame_at(&t, 2.5 / FPS), Some(2));
    assert_eq!(frame_at(&t, 3.0 / FPS), Some(3));
    assert_eq!(frame_at(&t, -1.0), Some(0));
    assert_eq!(frame_at(&t, 99.0), Some(9));
    assert_eq!(frame_at(&[], 1.0), None);
    let gaps = [(0.0, 0.1), (0.5, 0.6), (1.0, 1.1)];
    assert_eq!(frame_at(&gaps, 0.2), Some(0), "nearer the earlier start");
    assert_eq!(frame_at(&gaps, 0.4), Some(1), "nearer the later start");
}

#[test]
fn the_keyframe_quad_is_the_observation_at_the_keyframe_time() {
    let a = quad(0.0, 0.0, 10.0, 10.0);
    let b = quad(5.0, 0.0, 15.0, 10.0);
    let c = quad(9.0, 0.0, 19.0, 10.0);
    let frames = [frame(0.0, 1.0, a), frame(1.0, 2.0, b), frame(3.0, 4.0, c)];
    assert_eq!(keyframe_quad(&frames, 1.5), Some(b));
    assert_eq!(keyframe_quad(&frames, 2.2), Some(b), "nearest interval");
    assert_eq!(keyframe_quad(&frames, 2.9), Some(c));
    assert_eq!(keyframe_quad(&[], 1.0), None);
}

#[test]
fn static_writing_stays_within_half_a_pixel() {
    let key = quad(10.0, 10.0, 50.0, 30.0);
    let near = quad(10.4, 9.6, 50.3, 30.0);
    let far = quad(10.6, 10.0, 50.0, 30.0);
    assert!(is_static(
        &[frame(0.0, 1.0, key), frame(1.0, 2.0, near)],
        key
    ));
    assert!(!is_static(
        &[frame(0.0, 1.0, key), frame(1.0, 2.0, far)],
        key
    ));
}

#[test]
fn rectangles_grow_by_their_margin_and_stay_in_the_frame() {
    let q = quad(10.2, 20.7, 110.5, 60.0);
    assert_eq!(
        around(q, 4.0, 640, 360),
        Some(PixelRect {
            x: 6,
            y: 16,
            width: 109,
            height: 48
        })
    );
    assert_eq!(context_margin(q), 32.0);
    assert_eq!(context_margin(quad(0.0, 0.0, 400.0, 100.0)), 75.0);
    let clipped = around(q, 32.0, 100, 70).expect("a rectangle");
    assert_eq!(
        (clipped.x, clipped.y, clipped.right(), clipped.bottom()),
        (0, 0, 100, 70)
    );
    assert_eq!(around(quad(700.0, 10.0, 720.0, 20.0), 4.0, 640, 360), None);
}

#[test]
fn line_height_shares_the_shorter_side_among_lines() {
    let q = quad(0.0, 0.0, 200.0, 80.0);
    assert_eq!(line_height(q, "文字"), 80.0);
    assert_eq!(line_height(q, "一行\n二行"), 40.0);
    assert_eq!(line_height(q, ""), 80.0);
}
