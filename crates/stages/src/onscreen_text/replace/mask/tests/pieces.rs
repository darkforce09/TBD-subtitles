use super::{apart, frames, unwrapped};

const W: u32 = 60;
const H: u32 = 30;

/// Row-major pixels of a `W` × `H` window set inside any of `rects` (left, top, right, bottom).
fn pixels(rects: &[(u32, u32, u32, u32)]) -> Vec<bool> {
    (0..H)
        .flat_map(|y| (0..W).map(move |x| (x, y)))
        .map(|(x, y)| {
            rects
                .iter()
                .any(|&(l, t, r, b)| x >= l && x < r && y >= t && y < b)
        })
        .collect()
}

fn at(flags: &[bool], x: u32, y: u32) -> bool {
    flags[(y * W + x) as usize]
}

#[test]
fn a_ruled_border_around_the_writing_is_a_frame_and_the_writing_is_not() {
    let ink = pixels(&[
        (2, 2, 58, 4),
        (2, 26, 58, 28),
        (2, 2, 4, 28),
        (56, 2, 58, 28),
        (20, 10, 30, 20),
        (34, 10, 40, 20),
    ]);
    let frame = frames(&ink, W, H, 10.0);
    assert!(at(&frame, 30, 3) && at(&frame, 3, 15));
    assert!(!at(&frame, 25, 15) && !at(&frame, 36, 15));
}

#[test]
fn a_long_dense_line_of_writing_is_no_frame() {
    let ink = pixels(&[(2, 5, 58, 25)]);
    assert!(frames(&ink, W, H, 10.0).iter().all(|&f| !f));
}

#[test]
fn pieces_mostly_outside_a_loose_box_are_apart() {
    let ink = pixels(&[(10, 10, 20, 20), (40, 10, 50, 20)]);
    let inside = pixels(&[(0, 0, 42, 30)]);
    let beside = apart(&ink, &inside, W, H);
    assert!(!at(&beside, 15, 15), "wholly inside");
    assert!(at(&beside, 45, 15), "a fifth inside");
}

#[test]
fn a_fill_its_outline_does_not_ring_is_unwrapped() {
    let glyph = (10, 10, 20, 20);
    let patch = (35, 5, 55, 25);
    let fill = pixels(&[glyph, patch]);
    // The outline rings the glyph and touches only the patch's left side.
    let near = pixels(&[(8, 8, 22, 22), (33, 5, 37, 25)]);
    let loose = unwrapped(&fill, &near, W, H);
    assert!(!at(&loose, 15, 15));
    assert!(at(&loose, 45, 15));
}
