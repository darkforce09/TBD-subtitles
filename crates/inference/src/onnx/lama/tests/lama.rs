use super::*;

use crate::model_store;

#[test]
fn the_image_is_packed_as_planar_channels_in_unit_range() {
    let rgb = [0, 51, 255, 102, 153, 204];
    let planar = planar_image(&rgb);
    let expected = [0.0, 0.4, 0.2, 0.6, 1.0, 0.8];
    for (value, want) in planar.iter().zip(expected) {
        assert!((value - want).abs() < 1e-6, "{planar:?}");
    }
}

#[test]
fn any_non_zero_mask_byte_erases() {
    assert_eq!(binary_mask(&[0, 1, 128, 255]), vec![0.0, 1.0, 1.0, 1.0]);
}

#[test]
fn unpacking_rounds_clamps_and_keeps_unmasked_pixels() {
    let rgb = [10, 20, 30, 40, 50, 60, 70, 80, 90];
    let mask = [0, 255, 7];
    // Planar channels for three pixels; the first pixel's values must be ignored.
    let planar = [
        200.0,
        12.4,
        -3.0, // red
        200.0,
        12.6,
        300.0, // green
        200.0,
        f32::NAN,
        99.5, // blue
    ];
    let out = interleaved_pixels(&planar, 1.0, &rgb, &mask);
    assert_eq!(out, vec![10, 20, 30, 12, 13, 0, 0, 255, 100]);
}

#[test]
fn unit_range_output_is_scaled_by_its_levels() {
    let out = interleaved_pixels(&[0.5, 1.0, 0.0], 255.0, &[1, 2, 3], &[255]);
    assert_eq!(out, vec![128, 255, 0]);
}

#[test]
fn wrong_buffer_lengths_are_refused() {
    assert!(check_lengths(&[0; 3], &[0; 1]).is_err());
    assert!(check_lengths(&vec![0; SIDE * SIDE * 3], &vec![0; SIDE * SIDE]).is_ok());
}

#[test]
#[ignore = "needs the LaMa model and CUDA"]
fn lama_fills_strokes_from_the_surrounding_gradient() {
    let dir = model_store::models_dir().unwrap().join(MODEL);
    let mut lama = Lama::open(&dir, Device::Cuda).unwrap();
    let gradient = |x: usize, y: usize| [(x / 2) as u8, (y / 2) as u8, 128];
    let mut rgb = vec![0; SIDE * SIDE * 3];
    let mut mask = vec![0; SIDE * SIDE];
    for y in 0..SIDE {
        for x in 0..SIDE {
            let stroke = (200..312).contains(&y) && x % 40 < 6 || (250..256).contains(&y);
            let pixel = if stroke { [0, 0, 0] } else { gradient(x, y) };
            rgb[(y * SIDE + x) * 3..][..3].copy_from_slice(&pixel);
            mask[y * SIDE + x] = if stroke { 255 } else { 0 };
        }
    }
    let out = lama.inpaint(&rgb, &mask).unwrap();
    let (mut before, mut after, mut count) = (0.0, 0.0, 0.0);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let i = y * SIDE + x;
            if mask[i] == 0 {
                assert_eq!(out[i * 3..][..3], rgb[i * 3..][..3]);
                continue;
            }
            let want = gradient(x, y);
            for c in 0..3 {
                before += (f64::from(rgb[i * 3 + c]) - f64::from(want[c])).abs();
                after += (f64::from(out[i * 3 + c]) - f64::from(want[c])).abs();
            }
            count += 3.0;
        }
    }
    let (before, after) = (before / count, after / count);
    println!("mean error in the mask: {before:.1} before, {after:.1} after");
    assert!(after < before * 0.25, "before {before}, after {after}");
}
