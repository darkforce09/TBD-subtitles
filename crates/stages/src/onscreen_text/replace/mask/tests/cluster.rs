use super::{CLUSTER_COUNTS, distance, lab, pair_separation, partitions};

#[test]
fn lab_matches_reference_values() {
    let white = lab([255, 255, 255]);
    assert!((white[0] - 100.0).abs() < 0.1 && white[1].abs() < 0.1 && white[2].abs() < 0.1);
    assert_eq!(lab([0, 0, 0]), [0.0, 0.0, 0.0]);
    let red = lab([255, 0, 0]);
    assert!((red[0] - 53.24).abs() < 0.2, "{red:?}");
    assert!((red[1] - 80.09).abs() < 0.3, "{red:?}");
    assert!((red[2] - 67.20).abs() < 0.3, "{red:?}");
    let grey = lab([119, 119, 119]);
    assert!((grey[0] - 50.0).abs() < 0.5, "{grey:?}");
}

#[test]
fn three_flat_colours_split_into_three_clusters_at_every_larger_count() {
    let colours = [[60, 90, 150], [245, 240, 230], [20, 20, 30]];
    let pixels: Vec<_> = (0..300)
        .map(|i| {
            lab(colours[if i < 200 {
                0
            } else if i < 260 {
                1
            } else {
                2
            }])
        })
        .collect();
    let all = partitions(&pixels);
    assert_eq!(all.len(), CLUSTER_COUNTS.count());
    for clusters in &all[1..] {
        assert_eq!(clusters.centres.len(), 3);
        let mut counts = clusters.counts.clone();
        counts.sort_unstable();
        assert_eq!(counts, [40, 60, 200]);
        for (pixel, &label) in pixels.iter().zip(&clusters.labels) {
            assert!(distance(*pixel, clusters.centres[usize::from(label)]) < 1e-3);
        }
    }
}

#[test]
fn two_flat_colours_are_far_apart() {
    let pixels: Vec<_> = (0..100)
        .map(|i| {
            lab(if i < 70 {
                [10, 10, 10]
            } else {
                [250, 250, 250]
            })
        })
        .collect();
    let clusters = &partitions(&pixels)[0];
    assert_eq!(clusters.centres.len(), 2);
    assert!(pair_separation(clusters, 0, 1) > 10.0);
}

#[test]
fn large_regions_are_fitted_on_a_sample_and_labelled_whole() {
    let pixels: Vec<_> = (0..200_000)
        .map(|i| {
            lab(if i % 10 < 3 {
                [240, 240, 240]
            } else {
                [30, 60, 90]
            })
        })
        .collect();
    let clusters = &partitions(&pixels)[0];
    assert_eq!(clusters.labels.len(), pixels.len());
    let mut counts = clusters.counts.clone();
    counts.sort_unstable();
    assert_eq!(counts, [60_000, 140_000]);
}

#[test]
fn nothing_to_split_without_pixels() {
    assert!(partitions(&[]).is_empty());
    let one = partitions(&[lab([1, 2, 3]); 10]);
    assert!(one.iter().all(|c| c.centres.len() == 1));
}
