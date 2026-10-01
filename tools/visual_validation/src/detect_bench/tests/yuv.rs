use super::*;

#[test]
fn limited_range_black_white_and_grey_map_to_full_range() {
    assert_eq!(bt709(16, 128, 128), [0, 0, 0]);
    assert_eq!(bt709(235, 128, 128), [255, 255, 255]);
    assert_eq!(bt709(126, 128, 128), [128, 128, 128]);
}

#[test]
fn bt709_primaries_come_back_within_one_level() {
    // The BT.709 limited-range codes of pure red, green and blue.
    let close = |got: [u8; 3], want: [u8; 3]| {
        got.iter()
            .zip(want)
            .all(|(g, w)| (i32::from(*g) - i32::from(w)).abs() <= 1)
    };
    assert!(close(bt709(63, 102, 240), [255, 0, 0]), "red");
    assert!(close(bt709(173, 42, 26), [0, 255, 0]), "green");
    assert!(close(bt709(32, 240, 118), [0, 0, 255]), "blue");
}

/// A 6 × 4 yuv420p frame with varied samples, and the same frame as nv12.
fn frames() -> (Vec<u8>, Vec<u8>) {
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
    let (planar, nv12) = frames();
    let planar = Yuv420::planar(&planar, 6, 4).expect("a 6 × 4 yuv420p frame");
    let nv12 = Yuv420::nv12(&nv12, 6, 4).expect("a 6 × 4 nv12 frame");
    let mut serial = vec![0; planar.rgb_len()];
    let mut parallel = vec![0; planar.rgb_len()];
    let mut interleaved = vec![0; nv12.rgb_len()];
    assert!(to_rgb(&planar, &mut serial));
    assert!(to_rgb_parallel(&planar, &mut parallel));
    assert!(to_rgb_parallel(&nv12, &mut interleaved));
    assert_eq!(serial, parallel);
    assert_eq!(serial, interleaved);
    // Pixel (3, 2) shares the chroma sample at (1, 1): U 60 + 4·23, V 200 − 4·19.
    let at = (2 * 6 + 3) * 3;
    assert_eq!(&serial[at..at + 3], &bt709(16 + 15 * 9, 152, 124));
}

#[test]
fn odd_sizes_and_wrong_buffers_are_refused() {
    let (planar, _) = frames();
    assert!(Yuv420::planar(&planar, 5, 4).is_none());
    assert!(Yuv420::planar(&planar[1..], 6, 4).is_none());
    let frame = Yuv420::planar(&planar, 6, 4).expect("a 6 × 4 frame");
    assert!(!to_rgb(&frame, &mut [0; 10]));
    assert!(!to_rgb_parallel(&frame, &mut [0; 10]));
}
