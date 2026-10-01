use super::*;
use crate::yuv::{Coefficients, Matrix, Range};

fn bt709() -> Coefficients {
    Coefficients::new(Matrix::Bt709, Range::Limited)
}

#[test]
fn limited_range_black_white_and_grey_map_to_full_range() {
    let colour = bt709();
    assert_eq!(colour.rgb(16, 128, 128), [0, 0, 0]);
    assert_eq!(colour.rgb(235, 128, 128), [255, 255, 255]);
    assert_eq!(colour.rgb(126, 128, 128), [128, 128, 128]);
    let full = Coefficients::new(Matrix::Bt709, Range::Full);
    assert_eq!(full.rgb(0, 128, 128), [0, 0, 0]);
    assert_eq!(full.rgb(255, 128, 128), [255, 255, 255]);
}

/// Whether two colours differ by at most one level in every channel.
fn close(got: [u8; 3], want: [u8; 3]) -> bool {
    got.iter()
        .zip(want)
        .all(|(g, w)| (i32::from(*g) - i32::from(w)).abs() <= 1)
}

#[test]
fn each_matrix_brings_its_own_primaries_back_within_one_level() {
    // The limited-range codes of pure red, green and blue in each matrix.
    let cases = [
        (
            Matrix::Bt709,
            [(63, 102, 240), (173, 42, 26), (32, 240, 118)],
        ),
        (
            Matrix::Bt601,
            [(81, 90, 240), (145, 54, 34), (41, 240, 110)],
        ),
        (
            Matrix::Bt2020,
            [(74, 97, 240), (164, 47, 25), (29, 240, 119)],
        ),
    ];
    for (matrix, [red, green, blue]) in cases {
        let colour = Coefficients::new(matrix, Range::Limited);
        assert!(
            close(colour.rgb(red.0, red.1, red.2), [255, 0, 0]),
            "{matrix:?} red"
        );
        assert!(
            close(colour.rgb(green.0, green.1, green.2), [0, 255, 0]),
            "{matrix:?} green"
        );
        assert!(
            close(colour.rgb(blue.0, blue.1, blue.2), [0, 0, 255]),
            "{matrix:?} blue"
        );
    }
}

#[test]
fn a_stream_without_tags_takes_its_matrix_from_its_height() {
    assert_eq!(Matrix::from_tag(None, 1080), Matrix::Bt709);
    assert_eq!(Matrix::from_tag(None, 480), Matrix::Bt601);
    assert_eq!(Matrix::from_tag(Some("smpte170m"), 1080), Matrix::Bt601);
    assert_eq!(Range::from_tag(Some("pc")), Range::Full);
    assert_eq!(Range::from_tag(Some("tv")), Range::Limited);
}

/// A 6 × 4 yuv420p picture with varied samples, and the same picture as nv12.
fn pictures() -> (Vec<u8>, Vec<u8>) {
    let (width, height) = (6, 4);
    let luma: Vec<u8> = (0..width * height).map(|i| (16 + i * 9) as u8).collect();
    let u: Vec<u8> = (0..width * height / 4)
        .map(|i| (60 + i * 23) as u8)
        .collect();
    let v: Vec<u8> = (0..width * height / 4)
        .map(|i| (200 - i * 19) as u8)
        .collect();
    let planar = [luma.clone(), u.clone(), v.clone()].concat();
    let mut nv12 = luma;
    for (u, v) in u.iter().zip(&v) {
        nv12.extend([*u, *v]);
    }
    (planar, nv12)
}

#[test]
fn planar_and_interleaved_chroma_convert_to_the_same_picture_on_one_or_many_threads() {
    let (planar, nv12) = pictures();
    let planar = Yuv420::planar(&planar, 6, 4).expect("a 6 × 4 yuv420p picture");
    let nv12 = Yuv420::nv12(&nv12, 6, 4).expect("a 6 × 4 nv12 picture");
    let colour = bt709();
    let mut serial = vec![0; planar.rgb_len()];
    let mut parallel = vec![0; planar.rgb_len()];
    let mut interleaved = vec![0; nv12.rgb_len()];
    assert!(to_rgb(&planar, &colour, &mut serial));
    assert!(to_rgb_parallel(&planar, &colour, &mut parallel));
    assert!(to_rgb_parallel(&nv12, &colour, &mut interleaved));
    assert_eq!(serial, parallel);
    assert_eq!(serial, interleaved);
    // Pixel (3, 2) shares the chroma sample at (1, 1): U 60 + 4·23, V 200 − 4·19.
    let at = (2 * 6 + 3) * 3;
    assert_eq!(&serial[at..at + 3], &colour.rgb(16 + 15 * 9, 152, 124));
}

#[test]
fn a_padded_conversion_keeps_the_picture_on_top_and_blacks_the_rows_below() {
    let (planar, _) = pictures();
    let picture = Yuv420::planar(&planar, 6, 4).unwrap();
    let colour = bt709();
    let mut whole = vec![0; picture.rgb_len()];
    assert!(to_rgb(&picture, &colour, &mut whole));
    let mut padded = vec![7; 6 * 8 * 3];
    assert!(to_rgb_padded_into(&picture, &colour, 8, &mut padded));
    assert_eq!(&padded[..whole.len()], whole.as_slice());
    assert!(padded[whole.len()..].iter().all(|&byte| byte == 0));
    assert!(!to_rgb_padded_into(&picture, &colour, 2, &mut padded));
    assert!(!to_rgb_padded_into(&picture, &colour, 8, &mut padded[1..]));
}

#[test]
fn a_crop_equals_the_same_rectangle_of_the_whole_conversion() {
    let (planar, nv12) = pictures();
    let colour = bt709();
    for picture in [
        Yuv420::planar(&planar, 6, 4).unwrap(),
        Yuv420::nv12(&nv12, 6, 4).unwrap(),
    ] {
        let mut whole = vec![0; picture.rgb_len()];
        assert!(to_rgb(&picture, &colour, &mut whole));
        let rect = Rect {
            x: 1,
            y: 1,
            width: 3,
            height: 2,
        };
        let mut crop = Vec::new();
        assert!(crop_to_rgb(&picture, &colour, rect, &mut crop));
        let mut expected = Vec::new();
        for row in 1..3 {
            expected.extend_from_slice(&whole[(row * 6 + 1) * 3..(row * 6 + 4) * 3]);
        }
        assert_eq!(crop, expected);
        let outside = Rect { x: 4, ..rect };
        assert!(!crop_to_rgb(&picture, &colour, outside, &mut crop));
    }
}

#[test]
fn odd_sizes_and_wrong_buffers_are_refused() {
    let (planar, _) = pictures();
    assert!(Yuv420::planar(&planar, 5, 4).is_none());
    assert!(Yuv420::planar(&planar[1..], 6, 4).is_none());
    assert_eq!(picture_bytes(6, 4), Some(36));
    assert_eq!(picture_bytes(5, 4), None);
    let picture = Yuv420::planar(&planar, 6, 4).expect("a 6 × 4 picture");
    let colour = bt709();
    assert!(!to_rgb(&picture, &colour, &mut [0; 10]));
    assert!(!to_rgb_parallel(&picture, &colour, &mut [0; 10]));
}
