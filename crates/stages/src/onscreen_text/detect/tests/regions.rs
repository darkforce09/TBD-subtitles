use super::*;
use image::{Rgb, RgbImage};
use job_model::onscreen::Point;
use job_model::outputs::ShotCut;

fn rectangle(left: f64, top: f64, width: f64, height: f64) -> Quad {
    Quad([
        Point { x: left, y: top },
        Point {
            x: left + width,
            y: top,
        },
        Point {
            x: left + width,
            y: top + height,
        },
        Point {
            x: left,
            y: top + height,
        },
    ])
}

/// The grey picture a region's luma would give.
fn grey(image: &RgbImage) -> GrayImage {
    imageops::grayscale(image)
}

fn lettering(width: u32, height: u32) -> RgbImage {
    let mut image = RgbImage::from_pixel(width, height, Rgb([240; 3]));
    for left in (16..width.saturating_sub(20)).step_by(24) {
        for y in 8..height - 8 {
            for x in left..left + 4 {
                image.put_pixel(x, y, Rgb([20; 3]));
            }
        }
        for y in 8..12 {
            for x in left..left + 13 {
                image.put_pixel(x, y, Rgb([20; 3]));
            }
        }
    }
    image
}

fn observation(quad: Quad) -> Observation {
    Observation {
        quad,
        confidence: 0.95,
        surface_rgb: None,
    }
}

fn active(image: &RgbImage, quad: Quad, occurrence: usize) -> Active {
    Active {
        occurrence,
        quad,
        anchor: signature(&grey(image)),
        anchor_box: quad,
    }
}

/// Whether each active region's anchor still matches `image`, the picture at its anchor box.
fn unchanged(active: &[Active], image: &RgbImage) -> Vec<bool> {
    active
        .iter()
        .map(|prior| same_signature(&prior.anchor, &signature(&grey(image))))
        .collect()
}

#[test]
fn a_small_changed_glyph_is_not_diluted_by_a_long_line() {
    for width in [256, 2048] {
        let image = lettering(width, 48);
        let mut changed = image.clone();
        for x in 48..52 {
            for y in 20..35 {
                changed.put_pixel(x, y, Rgb([20; 3]));
            }
        }
        let old_a = imageops::resize(&image, 32, 16, imageops::FilterType::Triangle).into_raw();
        let old_b = imageops::resize(&changed, 32, 16, imageops::FilterType::Triangle).into_raw();
        assert!(
            old_a
                .iter()
                .zip(old_b)
                .map(|(a, b)| u64::from(a.abs_diff(b)))
                .sum::<u64>()
                < 18 * old_a.len() as u64
        );
        assert!(
            !same_signature(&signature(&grey(&image)), &signature(&grey(&changed))),
            "width {width}"
        );
    }
}

#[test]
fn immutable_anchor_stops_gradual_fades_from_drifting_into_different_content() {
    let original = lettering(256, 40);
    let quad = rectangle(0.0, 0.0, 256.0, 40.0);
    let prior = active(&original, quad, 7);
    let mut previous = original.clone();
    let mut split = false;
    for amount in (2..=20).step_by(2) {
        let mut current = original.clone();
        for pixel in current.pixels_mut() {
            for channel in &mut pixel.0 {
                *channel -= amount;
            }
        }
        assert!(
            same_signature(&signature(&grey(&previous)), &signature(&grey(&current))),
            "successive small fade"
        );
        if associate(
            std::slice::from_ref(&prior),
            &[observation(quad)],
            &unchanged(std::slice::from_ref(&prior), &current),
        )[0]
        .is_none()
        {
            split = true;
        }
        previous = current;
    }
    assert!(split, "fixed anchor must catch accumulated changes");
    assert_eq!(prior.anchor, signature(&grey(&original)));
}

#[test]
fn normal_translation_jitter_and_duplicate_frames_keep_the_same_occurrence() {
    let original = lettering(256, 40);
    let quad = rectangle(10.0, 10.0, 256.0, 40.0);
    let prior = active(&original, quad, 7);
    assert_eq!(
        associate(
            std::slice::from_ref(&prior),
            &[observation(quad)],
            &unchanged(std::slice::from_ref(&prior), &original),
        ),
        [Some(0)]
    );
    let mut jittered = RgbImage::from_pixel(256, 40, Rgb([240; 3]));
    for (x, y, pixel) in original.enumerate_pixels() {
        if x + 1 < 256 {
            jittered.put_pixel(x + 1, y, *pixel);
        }
    }
    let moved = rectangle(11.0, 10.0, 256.0, 40.0);
    assert_eq!(
        associate(
            std::slice::from_ref(&prior),
            &[observation(moved)],
            &unchanged(std::slice::from_ref(&prior), &jittered),
        ),
        [Some(0)]
    );
}

#[test]
fn scrolling_boundary_content_is_compared_instead_of_ignoring_shifted_edges() {
    let original = lettering(256, 40);
    let mut entered = original.clone();
    for y in 10..30 {
        for x in 0..3 {
            entered.put_pixel(x, y, Rgb([20; 3]));
        }
    }
    assert!(!same_signature(
        &signature(&grey(&original)),
        &signature(&grey(&entered))
    ));
}

#[test]
fn ambiguous_overlapping_regions_are_split_in_both_directions() {
    let image = lettering(256, 40);
    let quad = rectangle(0.0, 0.0, 256.0, 40.0);
    let priors = [active(&image, quad, 1), active(&image, quad, 2)];
    assert_eq!(
        associate(&priors, &[observation(quad)], &unchanged(&priors, &image)),
        [None]
    );
    assert_eq!(
        associate(
            &priors[..1],
            &[observation(quad), observation(quad)],
            &unchanged(&priors[..1], &image),
        ),
        [None, None]
    );
    let other = rectangle(300.0, 0.0, 256.0, 40.0);
    let priors = [active(&image, quad, 1), active(&image, other, 2)];
    assert_eq!(
        associate(
            &priors,
            &[observation(other), observation(quad)],
            &unchanged(&priors, &image),
        ),
        [Some(1), Some(0)]
    );
}

#[test]
fn brief_occurrences_preserve_exact_frame_intervals_and_cuts_break_continuity() {
    let observed = observation(rectangle(0.0, 0.0, 256.0, 40.0));
    let mut document = TextDocument::default();
    let index = start_occurrence(&mut document, 1.125, observed.confidence);
    append_observation(&mut document.occurrences[index], &observed, 1.125, 1.166667);
    assert_eq!(document.occurrences[index].frames.len(), 1);
    assert_eq!(
        (
            document.occurrences[index].start_s,
            document.occurrences[index].end_s
        ),
        (1.125, 1.166667)
    );
    let cuts = ShotChanges {
        cuts: vec![ShotCut {
            time_s: 1.166667,
            score: 30.0,
        }],
    };
    assert!(crosses_cut(&cuts, 1.125, 1.166667));
    assert!(!crosses_cut(&cuts, 1.166667, 1.208333));
    assert_eq!(
        associate(&[], &[observed], &[]),
        [None],
        "a cut clears all association candidates"
    );
}

#[test]
fn a_later_observation_ends_the_previous_frame_where_it_starts() {
    let observed = observation(rectangle(0.0, 0.0, 256.0, 40.0));
    let mut document = TextDocument::default();
    let index = start_occurrence(&mut document, 2.0, observed.confidence);
    let item = &mut document.occurrences[index];
    append_observation(item, &observed, 2.0, 2.0417);
    append_observation(item, &observed, 2.5, 2.5417);
    let times: Vec<_> = item.frames.iter().map(|f| (f.time_s, f.end_s)).collect();
    assert_eq!(times, [(2.0, 2.5), (2.5, 2.5417)]);
    assert_eq!((item.start_s, item.end_s), (2.0, 2.5417));
    assert_eq!(item.crops, [PathBuf::from("visual/crops/text-000001.png")]);
}

#[test]
fn the_occurrence_cap_allows_continuations_but_never_new_occurrences() {
    assert!(check_limits(OBSERVATION_LIMIT, OCCURRENCE_LIMIT, false).is_ok());
    assert!(check_limits(OBSERVATION_LIMIT, OCCURRENCE_LIMIT, true).is_err());
    assert!(check_limits(OBSERVATION_LIMIT + 1, 1, false).is_err());
    assert!(check_limits(OBSERVATION_LIMIT, OCCURRENCE_LIMIT - 1, true).is_ok());
}

#[test]
fn signatures_stay_bounded() {
    for (width, height) in [(2048, 48), (48, 2048), (1000, 1000), (20, 10)] {
        let signature = signature(&GrayImage::new(width, height));
        assert!(signature.width().max(signature.height()) <= 768);
        assert!(signature.width().min(signature.height()) <= 48);
        assert!(signature.width() <= width && signature.height() <= height);
    }
}

#[test]
fn overlap_is_intersection_over_union_of_the_bounds() {
    let a = rectangle(0.0, 0.0, 100.0, 20.0);
    assert!((overlap(a, a) - 1.0).abs() < 1e-9);
    assert!((overlap(a, rectangle(50.0, 0.0, 100.0, 20.0)) - 1.0 / 3.0).abs() < 1e-9);
    assert_eq!(overlap(a, rectangle(200.0, 0.0, 100.0, 20.0)), 0.0);
}
