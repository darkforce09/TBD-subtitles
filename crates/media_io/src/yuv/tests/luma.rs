use super::*;
use crate::yuv::{Coefficients, Matrix, Range, Yuv420};

/// A `width` × `height` yuv420p picture whose luma is `level` everywhere but `patch`.
fn picture(width: usize, height: usize, level: u8, patch: Option<(Rect, u8)>) -> Vec<u8> {
    let mut bytes = vec![128; width * height * 3 / 2];
    for row in 0..height {
        for column in 0..width {
            let inside = patch.is_some_and(|(rect, _)| {
                (rect.x..rect.x + rect.width).contains(&column)
                    && (rect.y..rect.y + rect.height).contains(&row)
            });
            bytes[row * width + column] = match patch {
                Some((_, value)) if inside => value,
                _ => level,
            };
        }
    }
    bytes
}

#[test]
fn edge_cells_average_only_the_pixels_they_cover() {
    let bytes = picture(10, 6, 100, None);
    let thumbnail = luma_thumbnail(&Yuv420::planar(&bytes, 10, 6).unwrap(), 4);
    assert_eq!((thumbnail.columns, thumbnail.rows), (3, 2));
    assert!(thumbnail.means.iter().all(|&mean| mean == 100));
}

#[test]
fn a_small_change_matches_and_a_block_of_new_writing_does_not() {
    let base = picture(64, 64, 60, None);
    let dimmer = picture(64, 64, 63, None);
    let sign = Rect {
        x: 8,
        y: 8,
        width: 16,
        height: 8,
    };
    let written = picture(64, 64, 60, Some((sign, 235)));
    let thumbnail = |bytes: &[u8]| luma_thumbnail(&Yuv420::planar(bytes, 64, 64).unwrap(), 8);
    assert!(thumbnail(&base).matches(&thumbnail(&dimmer), 4, 4));
    assert!(!thumbnail(&base).matches(&thumbnail(&written), 4, 4));
    let other_grid = luma_thumbnail(&Yuv420::planar(&base, 64, 64).unwrap(), 16);
    assert!(!thumbnail(&base).matches(&other_grid, 4, 4));
}

#[test]
fn a_grey_crop_maps_limited_luma_to_full_range() {
    let rect = Rect {
        x: 2,
        y: 2,
        width: 4,
        height: 2,
    };
    let bytes = picture(8, 8, 16, Some((rect, 235)));
    let picture = Yuv420::planar(&bytes, 8, 8).unwrap();
    let colour = Coefficients::new(Matrix::Bt709, Range::Limited);
    let mut grey = Vec::new();
    assert!(crop_grey(&picture, &colour, rect, &mut grey));
    assert_eq!(grey, vec![255; 8]);
    let wider = Rect { width: 6, ..rect };
    assert!(crop_grey(&picture, &colour, wider, &mut grey));
    assert_eq!(&grey[4..6], &[0, 0]);
    assert!(!crop_grey(
        &picture,
        &colour,
        Rect { x: 7, ..rect },
        &mut grey
    ));
}
