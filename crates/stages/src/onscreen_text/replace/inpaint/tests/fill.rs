use super::*;

const FILL: [u8; 3] = [250, 5, 250];

/// Fills every masked pixel with `FILL` (or with a per-call grey when `greys` is set) and keeps
/// every call's input.
struct Constant {
    side: usize,
    calls: Vec<(Vec<u8>, Vec<u8>)>,
    greys: Option<Vec<u8>>,
}

impl Constant {
    fn new(side: usize) -> Constant {
        Constant {
            side,
            calls: Vec::new(),
            greys: None,
        }
    }
}

impl Inpaint for Constant {
    fn side(&self) -> usize {
        self.side
    }

    fn inpaint(&mut self, rgb: &[u8], mask: &[u8]) -> TextResult<Vec<u8>> {
        assert_eq!(rgb.len(), self.side * self.side * 3);
        assert_eq!(mask.len(), self.side * self.side);
        let fill = match &self.greys {
            Some(greys) => [greys[self.calls.len()]; 3],
            None => FILL,
        };
        self.calls.push((rgb.to_vec(), mask.to_vec()));
        let mut out = rgb.to_vec();
        for (pixel, _) in mask.iter().enumerate().filter(|&(_, &m)| m > 0) {
            out[pixel * 3..][..3].copy_from_slice(&fill);
        }
        Ok(out)
    }
}

fn pattern(width: u32, height: u32) -> RgbImage {
    RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([
            ((x * 7 + y * 3) % 256) as u8,
            ((x * 5 + y * 11) % 256) as u8,
            ((x * 13) % 256) as u8,
        ])
    })
}

fn mask_where(width: u32, height: u32, set: impl Fn(u32, u32) -> bool) -> GrayImage {
    GrayImage::from_fn(width, height, |x, y| {
        image::Luma([if set(x, y) { 255 } else { 0 }])
    })
}

/// Every masked pixel holds `FILL`; every pixel further than one pixel from the mask is the
/// source's.
fn assert_filled_and_kept(source: &RgbImage, mask: &GrayImage, out: &RgbImage) {
    assert_eq!(out.dimensions(), source.dimensions());
    let near = dilate(mask);
    for (x, y, pixel) in out.enumerate_pixels() {
        if mask.get_pixel(x, y).0[0] > 0 {
            assert_eq!(pixel.0, FILL, "masked pixel ({x}, {y})");
        } else if near.get_pixel(x, y).0[0] == 0 {
            assert_eq!(pixel, source.get_pixel(x, y), "kept pixel ({x}, {y})");
        }
    }
}

#[test]
fn tile_positions_cover_the_axis_with_quarter_side_overlaps() {
    assert_eq!(tile_positions(64, 64), vec![0]);
    assert_eq!(tile_positions(10, 64), vec![0]);
    assert_eq!(tile_positions(100, 64), vec![0, 36]);
    assert_eq!(tile_positions(150, 64), vec![0, 43, 86]);
    for len in 513..3000 {
        let positions = tile_positions(len, 512);
        assert_eq!(positions[0], 0);
        assert_eq!(*positions.last().unwrap(), len - 512, "{len}");
        for pair in positions.windows(2) {
            assert!(pair[0] < pair[1]);
            assert!(pair[0] + 512 - pair[1] >= 128, "{len}: {positions:?}");
        }
    }
}

#[test]
fn the_working_size_depends_on_the_longest_side() {
    assert_eq!(working_size(40, 30, 64), (40, 30));
    assert_eq!(working_size(64, 64, 64), (64, 64));
    assert_eq!(working_size(100, 50, 64), (64, 32));
    assert_eq!(working_size(20, 128, 64), (10, 64));
    assert_eq!(working_size(129, 20, 64), (65, 10));
    assert_eq!(working_size(3000, 1, 512), (1500, 1));
}

#[test]
fn mirrored_indices_reflect_at_both_edges() {
    let folded: Vec<usize> = (0..8).map(|i| mirror(i, 3)).collect();
    assert_eq!(folded, vec![0, 1, 2, 2, 1, 0, 0, 1]);
    assert_eq!(mirror(5, 1), 0);
}

#[test]
fn a_pooled_mask_never_loses_a_stroke() {
    let dot = mask_where(100, 100, |x, y| x == 37 && y == 59);
    let pooled = max_pool(&dot, 64, 64);
    assert!(pooled.pixels().any(|p| p.0[0] == 255));
    let line = mask_where(300, 40, |x, _| x == 151);
    let pooled = max_pool(&line, 150, 20);
    for y in 0..20 {
        assert!(
            (0..150).any(|x| pooled.get_pixel(x, y).0[0] == 255),
            "row {y}"
        );
    }
}

#[test]
fn dilation_grows_a_pixel_to_its_eight_neighbours() {
    let dot = mask_where(5, 5, |x, y| x == 0 && y == 2);
    let grown = dilate(&dot);
    let set: Vec<(u32, u32)> = grown
        .enumerate_pixels()
        .filter(|(_, _, p)| p.0[0] > 0)
        .map(|(x, y, _)| (x, y))
        .collect();
    assert_eq!(set, vec![(0, 1), (1, 1), (0, 2), (1, 2), (0, 3), (1, 3)]);
}

#[test]
fn the_blend_feathers_over_one_pixel() {
    let source = RgbImage::from_pixel(5, 5, image::Rgb([0, 0, 0]));
    let filled = RgbImage::from_pixel(5, 5, image::Rgb([90, 90, 90]));
    let mask = mask_where(5, 5, |x, y| x == 2 && y == 2);
    let out = feathered_blend(&source, &filled, &mask);
    for (x, y, pixel) in out.enumerate_pixels() {
        let want = match (x.abs_diff(2), y.abs_diff(2)) {
            (0, 0) => 90,
            (dx, dy) if dx <= 1 && dy <= 1 => 10,
            _ => 0,
        };
        assert_eq!(pixel.0, [want; 3], "({x}, {y})");
    }
}

#[test]
fn a_plate_that_fits_is_padded_by_reflection_at_the_top_left() {
    let source = pattern(40, 30);
    let mask = mask_where(40, 30, |x, y| (10..20).contains(&x) && (5..15).contains(&y));
    let mut model = Constant::new(64);
    let out = fill_plate(&source, &mask, &mut model).unwrap();
    assert_eq!(model.calls.len(), 1);
    let (rgb, erase) = &model.calls[0];
    for ly in 0..64 {
        for lx in 0..64 {
            let (x, y) = (mirror(lx, 40) as u32, mirror(ly, 30) as u32);
            let i = ly * 64 + lx;
            assert_eq!(rgb[i * 3..][..3], source.get_pixel(x, y).0, "({lx}, {ly})");
            assert_eq!(erase[i], mask.get_pixel(x, y).0[0], "({lx}, {ly})");
        }
    }
    assert_filled_and_kept(&source, &mask, &out);
    for (x, y, pixel) in out.enumerate_pixels() {
        if mask.get_pixel(x, y).0[0] == 0 {
            assert_eq!(pixel, source.get_pixel(x, y));
        }
    }
}

#[test]
fn a_plate_up_to_twice_the_side_is_scaled_to_the_side() {
    let source = pattern(100, 50);
    let mask = mask_where(100, 50, |x, y| {
        (30..60).contains(&x) && (10..40).contains(&y) || x == 80 && (5..45).contains(&y)
    });
    let mut model = Constant::new(64);
    let out = fill_plate(&source, &mask, &mut model).unwrap();
    assert_eq!(model.calls.len(), 1);
    let (rgb, erase) = &model.calls[0];
    // The 64 × 32 working copy fills the top of the canvas; rows below mirror it.
    for lx in 0..64 {
        for (below, above) in [(32, 31), (40, 23), (63, 0)] {
            assert_eq!(
                rgb[(below * 64 + lx) * 3..][..3],
                rgb[(above * 64 + lx) * 3..][..3]
            );
            assert_eq!(erase[below * 64 + lx], erase[above * 64 + lx]);
        }
    }
    // The one-pixel stroke at x = 80 survives scaling.
    assert!((0..32).any(|ly| (48..54).any(|lx| erase[ly * 64 + lx] > 0)));
    assert_filled_and_kept(&source, &mask, &out);
}

#[test]
fn a_larger_plate_is_halved_and_tiled() {
    let source = pattern(300, 100);
    let mask = mask_where(300, 100, |x, y| {
        (10..290).contains(&x) && (40..60).contains(&y)
    });
    let mut model = Constant::new(64);
    let out = fill_plate(&source, &mask, &mut model).unwrap();
    // A 150 × 50 working copy: tiles at x = 0, 43 and 86, each padded below row 50.
    assert_eq!(model.calls.len(), 3);
    for (rgb, erase) in &model.calls {
        for lx in 0..64 {
            assert_eq!(
                rgb[(50 * 64 + lx) * 3..][..3],
                rgb[(49 * 64 + lx) * 3..][..3]
            );
            assert_eq!(erase[50 * 64 + lx], erase[49 * 64 + lx]);
        }
    }
    let (first, _) = &model.calls[0];
    let (second, _) = &model.calls[1];
    // The second tile starts 43 pixels into the first.
    assert_eq!(first[43 * 3..][..3], second[..3]);
    assert_filled_and_kept(&source, &mask, &out);
}

#[test]
fn overlapping_tiles_blend_linearly() {
    let image = pattern(150, 50);
    let mask = mask_where(150, 50, |_, _| true);
    let mut model = Constant::new(64);
    model.greys = Some(vec![0, 120, 240]);
    let out = fill_tiles(&image, &mask, &mut model).unwrap();
    let grey = |x: u32| out.get_pixel(x, 10).0[0];
    assert_eq!(grey(42), 0);
    assert_eq!(grey(64), 120);
    assert_eq!(grey(149), 240);
    let ramp: Vec<u8> = (43..64).map(grey).collect();
    assert!(ramp.windows(2).all(|pair| pair[0] <= pair[1]), "{ramp:?}");
    assert!(ramp[0] < 10 && ramp[20] > 110, "{ramp:?}");
}

#[test]
fn tiles_without_a_masked_pixel_cost_no_call() {
    let image = pattern(150, 50);
    let mask = mask_where(150, 50, |x, _| x < 20);
    let mut model = Constant::new(64);
    let out = fill_tiles(&image, &mask, &mut model).unwrap();
    assert_eq!(model.calls.len(), 1);
    for (x, y, pixel) in out.enumerate_pixels() {
        if x >= 20 {
            assert_eq!(pixel, image.get_pixel(x, y));
        }
    }
}

#[test]
fn a_wrong_model_answer_or_side_fails() {
    struct Short;
    impl Inpaint for Short {
        fn side(&self) -> usize {
            16
        }
        fn inpaint(&mut self, _: &[u8], _: &[u8]) -> TextResult<Vec<u8>> {
            Ok(vec![0; 3])
        }
    }
    let source = pattern(10, 10);
    let mask = mask_where(10, 10, |x, _| x == 3);
    assert!(fill_plate(&source, &mask, &mut Short).is_err());
    assert!(fill_plate(&source, &mask, &mut Constant::new(4)).is_err());
}
