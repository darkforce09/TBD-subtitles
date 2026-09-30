use super::*;

/// A 10 × 10 area on a 2× canvas with an opaque red square over area x 2..6, y 1..4.
fn canvas() -> (Canvas, Area) {
    let area = Area {
        width: 10.0,
        height: 10.0,
    };
    let mut canvas = Canvas {
        width: 20,
        height: 20,
        scale: 2.0,
        pixels: vec![[0; 4]; 400],
    };
    for y in 2..8 {
        for x in 4..12 {
            canvas.pixels[y * 20 + x] = [255, 0, 0, 255];
        }
    }
    (canvas, area)
}

fn quad(points: [(f64, f64); 4]) -> Quad {
    Quad(points.map(|(x, y)| Point { x, y }))
}

fn alpha(output: &[[f32; 4]], x: usize, y: usize) -> f32 {
    output[y * 10 + x][3]
}

#[test]
fn the_identity_quad_keeps_the_square_in_place() {
    let (canvas, area) = canvas();
    let target = quad([(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
    let output = warp(&canvas, area, target, 10, 10).unwrap();
    for y in 0..10 {
        for x in 0..10 {
            let expected = if (2..6).contains(&x) && (1..4).contains(&y) {
                1.0
            } else {
                0.0
            };
            assert!((alpha(&output, x, y) - expected).abs() < 1e-6, "({x}, {y})");
        }
    }
    assert!((output[2 * 10 + 3][0] - 1.0).abs() < 1e-6);
}

#[test]
fn a_quarter_turn_moves_the_square_with_the_quad() {
    let (canvas, area) = canvas();
    // The area's top edge runs down the plate's right side: (u, v) → (10 − v, u).
    let target = quad([(10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0)]);
    let output = warp(&canvas, area, target, 10, 10).unwrap();
    // u 2..6, v 1..4 → x 6..9, y 2..6.
    assert!((alpha(&output, 7, 3) - 1.0).abs() < 1e-6);
    assert!((alpha(&output, 8, 5) - 1.0).abs() < 1e-6);
    assert_eq!(alpha(&output, 3, 3), 0.0);
    assert_eq!(alpha(&output, 7, 8), 0.0);
}

#[test]
fn a_shifted_half_size_quad_scales_the_square_down() {
    let (canvas, area) = canvas();
    let target = quad([(4.0, 4.0), (9.0, 4.0), (9.0, 9.0), (4.0, 9.0)]);
    let output = warp(&canvas, area, target, 10, 10).unwrap();
    // u 2..6 → x 5..7; v 1..4 → y 4.5..6.
    assert!(alpha(&output, 5, 5) > 0.99);
    assert!(alpha(&output, 6, 5) > 0.99);
    assert_eq!(alpha(&output, 1, 1), 0.0);
    assert_eq!(alpha(&output, 8, 8), 0.0);
}

#[test]
fn a_folded_quad_has_no_mapping() {
    let (canvas, area) = canvas();
    let folded = quad([(0.0, 0.0), (10.0, 10.0), (10.0, 0.0), (0.0, 10.0)]);
    assert!(warp(&canvas, area, folded, 10, 10).is_none());
}
