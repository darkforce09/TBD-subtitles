use super::*;

fn frame(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 3]) -> PaddedFrame {
    let padded_height = PaddedFrame::padded(height);
    let mut rgb = vec![0u8; (width * padded_height * 3) as usize];
    for y in 0..height {
        for x in 0..width {
            let at = ((y * width + x) * 3) as usize;
            rgb[at..at + 3].copy_from_slice(&pixel(x, y));
        }
    }
    PaddedFrame {
        width,
        height,
        padded_height,
        rgb,
    }
}

fn rect(left: f64, top: f64, right: f64, bottom: f64) -> Quad {
    Quad([
        Point { x: left, y: top },
        Point { x: right, y: top },
        Point {
            x: right,
            y: bottom,
        },
        Point { x: left, y: bottom },
    ])
}

#[test]
fn a_frame_shrinks_by_three_to_the_mean_of_each_block() {
    let source = frame(6, 3, |x, _| if x < 3 { [30, 60, 90] } else { [0, 0, 255] });
    let small = shrink(&source, (2, 1));
    assert_eq!((small.width, small.height, small.padded_height), (2, 1, 32));
    assert_eq!(small.rgb.len(), 2 * 32 * 3);
    assert_eq!(&small.rgb[..6], &[30, 60, 90, 0, 0, 255]);
    assert!(
        small.rgb[6..].iter().all(|&byte| byte == 0),
        "padding is black"
    );
}

#[test]
fn a_block_averages_its_pixels_rounding_to_nearest() {
    let source = frame(2, 2, |x, y| [(x + 2 * y) as u8, 0, 255]);
    let small = shrink(&source, (1, 1));
    assert_eq!(
        &small.rgb[..3],
        &[2, 0, 255],
        "mean of 0, 1, 2, 3 is 1.5, rounded to 2"
    );
}

#[test]
fn a_1080p_frame_shrinks_to_the_proxy_size() {
    assert_eq!(crate::ocr::pool::proxy_size(1920, 1080), (640, 360));
    assert_eq!(crate::ocr::pool::proxy_size(1280, 720), (640, 360));
    assert_eq!(crate::ocr::pool::proxy_size(1440, 1080), (640, 480));
    assert_eq!(crate::ocr::pool::proxy_size(640, 360), (640, 360));
    let source = frame(1920, 1080, |_, _| [9, 9, 9]);
    let small = shrink(&source, (640, 360));
    assert_eq!((small.width, small.padded_height), (640, 384));
    assert!(small.rgb[..640 * 360 * 3].iter().all(|&byte| byte == 9));
}

#[test]
fn proxy_regions_scale_to_frame_pixels_and_stay_inside() {
    let found = vec![
        (rect(100.0, 50.0, 200.0, 90.0), 0.8),
        (rect(600.0, 300.0, 640.0, 360.0), 0.6),
    ];
    let scaled = scale_up(found, (640, 360), (1920, 1080));
    assert_eq!(scaled[0].0.bounds(), (300.0, 150.0, 600.0, 270.0));
    assert_eq!(scaled[0].1, 0.8);
    assert_eq!(scaled[1].0.bounds(), (1800.0, 900.0, 1920.0, 1080.0));
}

#[test]
fn a_proxy_region_found_at_full_resolution_is_left_out() {
    let mut full = vec![
        (rect(0.0, 0.0, 60.0, 20.0), 0.9),
        (rect(60.0, 0.0, 100.0, 20.0), 0.9),
    ];
    let proxy = vec![
        (rect(0.0, 0.0, 100.0, 22.0), 0.7),
        (rect(300.0, 300.0, 600.0, 600.0), 0.8),
    ];
    merge(&mut full, proxy);
    assert_eq!(full.len(), 3, "the line is covered, the large glyph is not");
    assert_eq!(full[2].0.bounds(), (300.0, 300.0, 600.0, 600.0));
}

#[test]
fn small_noise_inside_a_large_glyph_does_not_hide_it() {
    let mut full = vec![(rect(310.0, 310.0, 330.0, 330.0), 0.4)];
    merge(&mut full, vec![(rect(300.0, 300.0, 600.0, 600.0), 0.8)]);
    assert_eq!(full.len(), 2);
}
