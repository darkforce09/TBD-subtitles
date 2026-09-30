use image::{Rgba, RgbaImage};
use job_model::onscreen::PixelRect;
use media_io::video_frames::PixelFormat;

use super::*;
use crate::localize::colour::{Conversion, Matrix, Range};

const SIZE: (u32, u32) = (8, 8);

fn limited(bits: u32) -> Conversion {
    Conversion {
        matrix: Matrix::Bt709,
        range: Range::Limited,
        bits,
    }
}

fn rect(x: u32, y: u32, width: u32, height: u32) -> PixelRect {
    PixelRect {
        x,
        y,
        width,
        height,
    }
}

/// An 8-bit 4:2:0 frame with every luma sample `y` and every chroma sample `u` and `v`.
fn frame(y: u8, u: u8, v: u8) -> Vec<u8> {
    let (w, h) = (SIZE.0 as usize, SIZE.1 as usize);
    let mut bytes = vec![y; w * h];
    bytes.extend(std::iter::repeat_n(u, w * h / 4));
    bytes.extend(std::iter::repeat_n(v, w * h / 4));
    bytes
}

/// A 10-bit 4:2:0 frame of little-endian words.
fn wide_frame(y: u16, u: u16, v: u16) -> Vec<u8> {
    let (w, h) = (SIZE.0 as usize, SIZE.1 as usize);
    let samples = std::iter::repeat_n(y, w * h)
        .chain(std::iter::repeat_n(u, w * h / 4))
        .chain(std::iter::repeat_n(v, w * h / 4));
    samples.flat_map(u16::to_le_bytes).collect()
}

fn luma(frame: &[u8], x: usize, y: usize) -> u8 {
    frame[y * SIZE.0 as usize + x]
}

/// The (Cb, Cr) of chroma block (bx, by) in an 8-bit frame.
fn chroma(frame: &[u8], bx: usize, by: usize) -> (u8, u8) {
    let luma_len = (SIZE.0 * SIZE.1) as usize;
    let quarter = luma_len / 4;
    let at = by * (SIZE.0 as usize / 2) + bx;
    (frame[luma_len + at], frame[luma_len + quarter + at])
}

fn wide_sample(frame: &[u8], index: usize) -> u16 {
    u16::from_le_bytes([frame[2 * index], frame[2 * index + 1]])
}

fn solid(width: u32, height: u32, rgba: [u8; 4]) -> RgbaImage {
    RgbaImage::from_pixel(width, height, Rgba(rgba))
}

#[test]
fn a_transparent_patch_leaves_every_byte_identical() {
    let mut pixels: Vec<u8> = (0..96u8).map(|v| v.wrapping_mul(37)).collect();
    let before = pixels.clone();
    let patch = convert(&solid(5, 3, [255, 0, 0, 0]), rect(1, 3, 5, 3), limited(8)).unwrap();
    blend(&mut pixels, PixelFormat::Yuv420p, SIZE, &patch).unwrap();
    assert_eq!(pixels, before);
}

#[test]
fn an_opaque_patch_writes_its_values_exactly_inside_and_nothing_outside() {
    let mut pixels = frame(16, 128, 128);
    let patch = convert(&solid(4, 4, [255, 0, 0, 255]), rect(2, 2, 4, 4), limited(8)).unwrap();
    blend(&mut pixels, PixelFormat::Yuv420p, SIZE, &patch).unwrap();
    for y in 0..8 {
        for x in 0..8 {
            let inside = (2..6).contains(&x) && (2..6).contains(&y);
            assert_eq!(luma(&pixels, x, y), if inside { 63 } else { 16 }, "{x},{y}");
        }
    }
    for by in 0..4 {
        for bx in 0..4 {
            let inside = (1..3).contains(&bx) && (1..3).contains(&by);
            let expected = if inside { (102, 240) } else { (128, 128) };
            assert_eq!(chroma(&pixels, bx, by), expected, "{bx},{by}");
        }
    }
}

#[test]
fn half_alpha_mixes_luma_halfway() {
    let mut pixels = frame(16, 128, 128);
    let patch = convert(
        &solid(2, 2, [255, 255, 255, 128]),
        rect(0, 0, 2, 2),
        limited(8),
    )
    .unwrap();
    blend(&mut pixels, PixelFormat::Yuv420p, SIZE, &patch).unwrap();
    // (128·235 + 127·16 + 127) / 255
    assert_eq!(luma(&pixels, 0, 0), 126);
    assert_eq!(luma(&pixels, 2, 0), 16);
}

#[test]
fn an_odd_rectangle_blends_chroma_by_the_share_of_each_block_it_covers() {
    let mut pixels = frame(16, 100, 100);
    let patch = convert(&solid(3, 3, [255, 0, 0, 255]), rect(1, 1, 3, 3), limited(8)).unwrap();
    assert_eq!((patch.chroma.x, patch.chroma.y), (0, 0));
    assert_eq!((patch.chroma.columns, patch.chroma.rows), (2, 2));
    assert_eq!(patch.chroma.coverage, vec![255, 510, 510, 1020]);
    blend(&mut pixels, PixelFormat::Yuv420p, SIZE, &patch).unwrap();
    // A quarter of the way: 100.5 rounds up to 101 and 135 stays.
    assert_eq!(chroma(&pixels, 0, 0), (101, 135));
    // Halfway: 101 and 170.
    assert_eq!(chroma(&pixels, 1, 0), (101, 170));
    assert_eq!(chroma(&pixels, 0, 1), (101, 170));
    // Whole: the patch value.
    assert_eq!(chroma(&pixels, 1, 1), (102, 240));
    assert_eq!(chroma(&pixels, 2, 1), (100, 100));
    assert_eq!(luma(&pixels, 0, 0), 16);
    assert_eq!(luma(&pixels, 1, 1), 63);
    assert_eq!(luma(&pixels, 3, 3), 63);
    assert_eq!(luma(&pixels, 4, 3), 16);
}

#[test]
fn chroma_is_the_alpha_weighted_colour_of_the_block() {
    let mut image = solid(2, 2, [0, 0, 0, 0]);
    image.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
    image.put_pixel(1, 1, Rgba([0, 0, 255, 255]));
    let patch = convert(&image, rect(0, 0, 2, 2), limited(8)).unwrap();
    // Red (102, 240) and blue (240, 118) averaged, the transparent pixels not counting.
    assert_eq!(patch.chroma.cb, vec![171]);
    assert_eq!(patch.chroma.cr, vec![179]);
    assert_eq!(patch.chroma.coverage, vec![510]);
}

#[test]
fn ten_bit_frames_blend_little_endian_words() {
    let mut pixels = wide_frame(64, 512, 512);
    let mut image = solid(2, 2, [255, 255, 255, 255]);
    image.put_pixel(1, 0, Rgba([255, 255, 255, 128]));
    let patch = convert(&image, rect(2, 0, 2, 2), limited(10)).unwrap();
    blend(&mut pixels, PixelFormat::Yuv420p10le, SIZE, &patch).unwrap();
    assert_eq!(wide_sample(&pixels, 2), 940);
    // (128·940 + 127·64 + 127) / 255
    assert_eq!(wide_sample(&pixels, 3), 504);
    assert_eq!(wide_sample(&pixels, 4), 64);
    assert_eq!(wide_sample(&pixels, 8 + 2), 940);
    let luma_len = 64;
    assert_eq!(wide_sample(&pixels, luma_len + 1), 512);
    assert_eq!(wide_sample(&pixels, luma_len), 512);
}

#[test]
fn a_patch_of_the_wrong_size_is_refused() {
    let error = convert(&solid(3, 3, [0, 0, 0, 255]), rect(0, 0, 4, 4), limited(8)).unwrap_err();
    assert!(error.0.contains("3x3"), "{error}");
    assert!(error.0.contains("4x4"), "{error}");
}

#[test]
fn blend_refuses_rgb_frames_short_frames_and_patches_outside_the_frame() {
    let patch = convert(&solid(2, 2, [0, 0, 0, 255]), rect(0, 0, 2, 2), limited(8)).unwrap();
    let mut rgb = vec![0; 8 * 8 * 3];
    assert!(blend(&mut rgb, PixelFormat::Rgb24, SIZE, &patch).is_err());
    let mut short = vec![0; 10];
    assert!(blend(&mut short, PixelFormat::Yuv420p, SIZE, &patch).is_err());
    let outside = convert(&solid(2, 2, [0, 0, 0, 255]), rect(7, 0, 2, 2), limited(8)).unwrap();
    let mut pixels = frame(16, 128, 128);
    assert!(blend(&mut pixels, PixelFormat::Yuv420p, SIZE, &outside).is_err());
}
