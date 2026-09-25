use super::*;

fn spans(pairs: &[(f64, f64)]) -> Vec<TimeSpan> {
    pairs.iter().map(|&(a, b)| TimeSpan::new(a, b)).collect()
}

#[test]
fn short_regions_group_until_the_minimum_then_cut_in_a_silence() {
    // Ten 4 s lines with 1 s pauses.
    let regions: Vec<TimeSpan> = (0..10)
        .map(|i| TimeSpan::new(i as f64 * 5.0, i as f64 * 5.0 + 4.0))
        .collect();
    let chunks = cut(&regions, &[], 0.016, &ChunkSettings::default());
    assert_eq!(chunks, spans(&[(0.0, 24.0), (25.0, 49.0)]));
}

#[test]
fn no_chunk_passes_the_maximum() {
    let regions: Vec<TimeSpan> = (0..40)
        .map(|i| TimeSpan::new(i as f64 * 2.0, i as f64 * 2.0 + 1.6))
        .collect();
    let settings = ChunkSettings {
        min_s: 100.0,
        ..ChunkSettings::default()
    };
    for chunk in cut(&regions, &[], 0.016, &settings) {
        assert!(chunk.duration_s() <= 60.0, "{chunk:?}");
    }
}

#[test]
fn a_long_pause_always_ends_a_chunk() {
    let chunks = cut(
        &spans(&[(0.0, 5.0), (10.0, 15.0)]),
        &[],
        0.016,
        &ChunkSettings::default(),
    );
    assert_eq!(chunks.len(), 2);
}

#[test]
fn a_region_longer_than_the_maximum_splits_at_its_quietest_frame() {
    let frame_s = 0.5;
    let mut scores = vec![0.9f32; 300];
    scores[150] = 0.1; // 75 s
    let chunks = cut(
        &spans(&[(0.0, 150.0)]),
        &scores,
        frame_s,
        &ChunkSettings::default(),
    );
    assert_eq!(chunks.len(), 3, "{chunks:?}");
    assert!(
        (chunks[0].end_s - chunks[1].start_s).abs() < 1e-9,
        "pieces touch"
    );
    assert!(chunks.iter().all(|c| c.duration_s() <= 60.0));
}

#[test]
fn chunks_are_ordered_and_disjoint() {
    let regions: Vec<TimeSpan> = (0..100)
        .map(|i| TimeSpan::new(i as f64 * 3.0, i as f64 * 3.0 + 2.5))
        .collect();
    let chunks = cut(&regions, &[], 0.016, &ChunkSettings::default());
    for pair in chunks.windows(2) {
        assert!(pair[0].end_s <= pair[1].start_s);
    }
}
