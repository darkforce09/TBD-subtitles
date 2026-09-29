use super::*;

fn point(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn rectangle() -> Quad {
    Quad([
        point(200.0, 100.0),
        point(1000.0, 100.0),
        point(1000.0, 700.0),
        point(200.0, 700.0),
    ])
}

/// A known camera transform, evaluated independently of the projection implementation.
fn moved(p: Point) -> Point {
    let divisor = 0.0002 * p.x - 0.00008 * p.y + 1.0;
    point(
        (1.08 * p.x + 0.07 * p.y + 23.0) / divisor,
        (-0.03 * p.x + 0.96 * p.y + 31.0) / divisor,
    )
}

fn source_grid() -> Vec<Point> {
    [100.0, 300.0, 500.0, 700.0]
        .into_iter()
        .flat_map(|y| [200.0, 400.0, 600.0, 800.0, 1000.0].map(|x| point(x, y)))
        .collect()
}

fn assert_point(found: Point, expected: Point, tolerance: f64) {
    let error = (found.x - expected.x).hypot(found.y - expected.y);
    assert!(error <= tolerance, "{found:?} vs {expected:?}: {error}");
}

#[test]
fn robust_fit_recovers_a_known_perspective_transform_despite_outliers() {
    let source = source_grid();
    let mut target: Vec<_> = source.iter().copied().map(moved).collect();
    let outliers = [1, 6, 11, 16, 19];
    for &index in &outliers {
        target[index].x += 150.0 + index as f64;
        target[index].y -= 90.0;
    }
    let fit = robust_fit(&source, &target, 2.0).expect("15 consistent features");
    let expected: Vec<_> = (0..source.len())
        .filter(|i| !outliers.contains(i))
        .collect();
    assert_eq!(fit.inliers, expected);
    assert!(fit.max_error < 1e-6);
    assert!(fit.rms_error < 1e-6);
    for point in [point(325.0, 215.0), point(735.0, 615.0)] {
        assert_point(project(&fit.homography, point).unwrap(), moved(point), 1e-6);
    }
    let again = robust_fit(&source, &target, 2.0).unwrap();
    assert_eq!(again.inliers, fit.inliers);
    assert_eq!(again.homography, fit.homography);
}

#[test]
fn noisy_inliers_remain_within_two_pixels_at_1080p() {
    let source = source_grid();
    let target: Vec<_> = source
        .iter()
        .enumerate()
        .map(|(index, &p)| {
            let exact = moved(p);
            point(
                exact.x + (index % 3) as f64 * 0.2 - 0.2,
                exact.y + (index % 4) as f64 * 0.1 - 0.15,
            )
        })
        .collect();
    let fit = robust_fit(&source, &target, 2.0).expect("subpixel evidence");
    assert_eq!(fit.inliers.len(), source.len());
    assert!(fit.max_error < 0.5);
    assert!(fit.rms_error > 0.01 && fit.rms_error < 0.5);
    for p in rectangle().0 {
        assert_point(project(&fit.homography, p).unwrap(), moved(p), 0.5);
    }
}

#[test]
fn collinear_coincident_and_insufficient_correspondences_are_rejected() {
    let line: Vec<_> = (0..12)
        .map(|i| point(137.3 + i as f64 * 91.7, 31.17 + i as f64 * 27.51))
        .collect();
    let target: Vec<_> = line.iter().copied().map(moved).collect();
    assert!(robust_fit(&line, &target, 2.0).is_none());
    assert!(robust_fit(&[point(10.0, 20.0); 8], &[point(40.0, 50.0); 8], 2.0).is_none());
    assert!(robust_fit(&line[..3], &target[..3], 2.0).is_none());
    assert!(robust_fit(&line, &target[..4], 2.0).is_none());
    let source = source_grid();
    let collapsed: Vec<_> = source.iter().map(|p| point(p.x, 50.0)).collect();
    assert!(robust_fit(&source, &collapsed, 2.0).is_none());
}

#[test]
fn invalid_numbers_and_tolerances_do_not_produce_a_fit() {
    let source = source_grid();
    let target: Vec<_> = source.iter().copied().map(moved).collect();
    for tolerance in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(robust_fit(&source, &target, tolerance).is_none());
    }
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut bad = source.clone();
        bad[5].x = invalid;
        assert!(robust_fit(&bad, &target, 2.0).is_none());
        assert!(robust_fit(&target, &bad, 2.0).is_none());
    }
}

#[test]
fn quad_mapping_matches_corners_and_interior_under_perspective() {
    let source = rectangle();
    let target = Quad(source.0.map(moved));
    let homography = quad_to_quad(source, target).expect("convex sign");
    let mapped = map_quad(&homography, source).unwrap();
    for (found, expected) in mapped.0.into_iter().zip(target.0) {
        assert_point(found, expected, 1e-6);
    }
    let middle = point(600.0, 400.0);
    assert_point(project(&homography, middle).unwrap(), moved(middle), 1e-6);
    assert!(contains(target, moved(middle)));
    assert!(!contains(target, moved(point(100.0, 50.0))));
    assert!((area(source) - 480_000.0).abs() < 1e-6);
}

#[test]
fn folded_reversed_and_horizon_crossing_surfaces_are_rejected() {
    let source = rectangle();
    let [a, b, c, d] = source.0;
    assert!(quad_to_quad(source, Quad([a, d, c, b])).is_none());
    assert!(quad_to_quad(source, Quad([a, c, b, d])).is_none());
    assert!(quad_to_quad(Quad([a, b, b, d]), source).is_none());
    let reflection = Homography::new(-1.0, 0.0, 1200.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);
    assert!(map_quad(&reflection, source).is_none());
    let horizon = Homography::new(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.002, 0.0, -1.0);
    assert!(map_quad(&horizon, source).is_none());
    assert!(project(&horizon, point(500.0, 100.0)).is_none());
}

#[test]
fn projection_and_containment_never_accept_nonfinite_coordinates() {
    let transform = Homography::identity();
    assert!(project(&transform, point(f64::NAN, 0.0)).is_none());
    let overflow = Homography::new(1.0, 0.0, 1e308, 0.0, 1.0, 0.0, 0.0, 0.0, 1e-8);
    assert!(project(&overflow, point(0.0, 0.0)).is_none());
    assert!(!contains(rectangle(), point(f64::NAN, 300.0)));
}

#[test]
fn independently_fitted_forward_and_backward_motion_have_subpixel_round_trip_error() {
    let source = source_grid();
    let target: Vec<_> = source.iter().copied().map(moved).collect();
    let forward = robust_fit(&source, &target, 2.0).unwrap();
    let backward = robust_fit(&target, &source, 2.0).unwrap();
    for original in source {
        let tracked = project(&forward.homography, original).unwrap();
        let returned = project(&backward.homography, tracked).unwrap();
        assert!(distance(original, returned) < 1e-6);
        let inconsistent = point(returned.x + 3.0, returned.y + 4.0);
        assert!((distance(original, inconsistent) - 5.0).abs() < 1e-6);
        assert!(distance(original, inconsistent) > 2.0);
    }
}
