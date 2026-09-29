use super::*;

fn alpha_at(pixels: &[u8], size: u32, x: u32, y: u32) -> u8 {
    pixels[((y * size + x) * 4 + 3) as usize]
}

#[test]
fn the_pixels_fill_a_square_of_four_bytes_each() {
    for size in [1, 16, 64, 256] {
        assert_eq!(rgba(size).len(), (size * size * 4) as usize);
    }
    assert!(rgba(0).is_empty());
}

#[test]
fn the_corners_are_transparent_and_the_centre_opaque() {
    let size = 64;
    let pixels = rgba(size);
    for (x, y) in [(0, 0), (size - 1, 0), (0, size - 1), (size - 1, size - 1)] {
        assert_eq!(alpha_at(&pixels, size, x, y), 0, "corner {x},{y}");
        let at = ((y * size + x) * 4) as usize;
        assert_eq!(
            &pixels[at..at + 3],
            &[0, 0, 0],
            "corner {x},{y} carries no colour"
        );
    }
    assert_eq!(alpha_at(&pixels, size, 32, 32), 255);
}

#[test]
fn the_same_size_gives_the_same_pixels() {
    assert_eq!(rgba(48), rgba(48));
}
