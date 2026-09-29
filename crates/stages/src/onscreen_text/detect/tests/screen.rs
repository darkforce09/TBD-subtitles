use super::*;
use image::Rgb;
use job_model::outputs::ShotCut;

fn timeline(count: u64) -> Vec<(f64, f64)> {
    (0..count)
        .map(|i| (i as f64 / 24.0, (i + 1) as f64 / 24.0))
        .collect()
}

fn frame(index: u64) -> ProxyFrame {
    ProxyFrame {
        index,
        time_s: index as f64 / 24.0,
        end_s: (index + 1) as f64 / 24.0,
        rgb: RgbImage::new(1, 1),
    }
}

fn picture() -> RgbImage {
    RgbImage::from_fn(64, 36, |x, y| {
        Rgb([(x * 4) as u8, (y * 7) as u8, ((x + y) * 3) as u8])
    })
}

fn ceil_log2(k: u64) -> usize {
    if k <= 1 {
        0
    } else {
        ((k - 1).ilog2() + 1) as usize
    }
}

#[test]
fn a_repeated_picture_is_a_near_duplicate_until_one_block_changes() {
    let proxy = picture();
    assert!(near_duplicate(&proxy, &proxy.clone()));
    let mut noisy = proxy.clone();
    for (i, pixel) in noisy.pixels_mut().enumerate() {
        for channel in &mut pixel.0 {
            *channel = if i % 2 == 0 {
                channel.saturating_add(3)
            } else {
                channel.saturating_sub(3)
            };
        }
    }
    assert!(near_duplicate(&proxy, &noisy), "sensor-level noise repeats");
    let mut changed = proxy.clone();
    for y in 0..32 {
        for x in 32..64 {
            let pixel = changed.get_pixel_mut(x, y);
            pixel.0[0] = pixel.0[0].wrapping_add(60);
        }
    }
    assert!(!near_duplicate(&proxy, &changed), "one changed block");
    let mut glyph = proxy.clone();
    for y in 33..36 {
        for x in 0..8 {
            glyph.put_pixel(x, y, Rgb([255; 3]));
        }
    }
    assert!(
        !near_duplicate(&proxy, &glyph),
        "a small glyph in a partial block"
    );
    assert!(!near_duplicate(&proxy, &RgbImage::new(32, 36)));
}

#[test]
fn bisection_finds_every_entry_and_exit_within_ceil_log2_k_probes() {
    for k in [1u64, 5, 12, 25] {
        for seek in [Seek::Entry, Seek::Exit] {
            for change in 1..=k {
                let base = 10 * k;
                let mut searches = [Search::new(base, base + k, seek)];
                let probes = std::cell::Cell::new(0usize);
                bisect(
                    &mut searches,
                    &mut (),
                    |_, indices| {
                        probes.set(probes.get() + indices.len());
                        Ok(())
                    },
                    |_, _, index| match seek {
                        Seek::Entry => index >= base + change,
                        Seek::Exit => index < base + change,
                    },
                )
                .unwrap();
                assert_eq!(searches[0].hi, base + change, "k {k}, {seek:?} at {change}");
                assert!(
                    probes.get() <= ceil_log2(k),
                    "k {k}: {} probes",
                    probes.get()
                );
            }
        }
    }
}

#[test]
fn transitions_in_different_gaps_advance_in_lockstep_with_one_screen_per_step() {
    let mut searches = [
        Search::new(0, 12, Seek::Entry),
        Search::new(24, 36, Seek::Exit),
        Search::new(48, 60, Seek::Entry),
        Search::new(0, 12, Seek::Entry),
    ];
    let changes = [5, 31, 60, 5];
    let mut calls: Vec<Vec<u64>> = Vec::new();
    bisect(
        &mut searches,
        &mut calls,
        |calls, indices| {
            calls.push(indices.to_vec());
            Ok(())
        },
        |calls, search, index| {
            assert!(
                calls.last().unwrap().contains(&index),
                "probed in this step"
            );
            match search {
                1 => index < changes[search],
                _ => index >= changes[search],
            }
        },
    )
    .unwrap();
    let found: Vec<u64> = searches.iter().map(|search| search.hi).collect();
    assert_eq!(found, changes);
    assert_eq!(calls.len(), ceil_log2(12), "one screening call per step");
    assert_eq!(calls[0], [6, 30, 54], "one deduplicated probe per gap");
    for call in &calls {
        let gaps: Vec<u64> = call.iter().map(|index| index / 12).collect();
        let mut distinct = gaps.clone();
        distinct.dedup();
        assert_eq!(gaps, distinct, "at most one probe per gap here: {call:?}");
    }
    assert_eq!(calls[3], [5, 59]);
}

#[test]
fn samples_are_every_kth_frame_both_frames_around_each_cut_and_the_final_frame() {
    let frames = timeline(100);
    let cuts = ShotChanges {
        cuts: vec![
            ShotCut {
                time_s: frames[50].0 - 0.001,
                score: 40.0,
            },
            ShotCut {
                time_s: frames[77].0,
                score: 60.0,
            },
            ShotCut {
                time_s: 99.0,
                score: 60.0,
            },
        ],
    };
    let samples = Samples::new(&frames, &cuts, 12);
    let expected: Vec<u64> = (0..100)
        .filter(|i| i % 12 == 0 || [49, 50, 76, 77, 99].contains(i))
        .collect();
    let found: Vec<u64> = (0..100).filter(|&i| samples.contains(i)).collect();
    assert_eq!(found, expected);
    assert_eq!(sample_step(24.0), 12);
    assert_eq!(sample_step(23.976), 12);
    assert_eq!(sample_step(29.97), 15);
    assert_eq!(sample_step(1.0), 1);
    assert_eq!(sample_step(0.0), 1);
}

#[test]
fn the_pending_batch_never_holds_more_than_eight_samples_with_their_gaps() {
    for k in [1u64, 5, 12, 25] {
        let frames = timeline(400);
        let cuts = ShotChanges {
            cuts: vec![ShotCut {
                time_s: frames[133].0,
                score: 50.0,
            }],
        };
        let samples = Samples::new(&frames, &cuts, k);
        let bound = SCREEN_BATCH * (k as usize + 1);
        let mut pending = Pending::default();
        let mut held = 0;
        for index in 0..400 {
            if let Some(batch) = pending.push(frame(index), samples.contains(index)) {
                assert_eq!(batch.len(), SCREEN_BATCH);
                let size: usize = batch.iter().map(|sample| 1 + sample.gap.len()).sum();
                assert!(size <= bound, "k {k}: batch of {size} frames");
                for sample in &batch {
                    assert!(
                        sample.gap.len() < k as usize,
                        "a gap holds at most k - 1 frames"
                    );
                    if sample.frame.index > 0 {
                        assert_eq!(
                            sample.previous_sample() + sample.gap.len() as u64 + 1,
                            sample.frame.index
                        );
                    }
                    for frame in &sample.gap {
                        assert_eq!(sample.frame(frame.index).unwrap().index, frame.index);
                    }
                }
                held += size;
            }
            assert!(
                pending.frames() <= bound,
                "k {k}: {} frames",
                pending.frames()
            );
        }
        held += pending.frames();
        let rest = pending.finish().unwrap();
        assert!(rest.len() < SCREEN_BATCH);
        assert_eq!(held, 400, "every frame passes through exactly one batch");
    }
}

#[test]
fn a_stream_that_stops_between_samples_is_an_error() {
    let mut pending = Pending::default();
    assert!(pending.push(frame(0), true).is_none());
    assert!(pending.push(frame(1), false).is_none());
    assert!(pending.finish().is_err());
}
