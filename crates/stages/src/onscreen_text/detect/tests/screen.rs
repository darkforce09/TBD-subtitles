use super::*;
use job_model::outputs::ShotCut;
use media_io::yuv::{Matrix, Range};

fn timeline(count: u64) -> Vec<(f64, f64)> {
    (0..count)
        .map(|i| (i as f64 / 24.0, (i + 1) as f64 / 24.0))
        .collect()
}

/// A 64 by 36 yuv420p picture with a luma gradient and neutral chroma.
fn picture() -> Vec<u8> {
    let mut bytes: Vec<u8> = (0..36u32)
        .flat_map(|y| (0..64u32).map(move |x| ((x * 3 + y * 5) % 200 + 20) as u8))
        .collect();
    bytes.resize(64 * 36 * 3 / 2, 128);
    bytes
}

fn yuv(bytes: &[u8]) -> Yuv420<'_> {
    Yuv420::planar(bytes, 64, 36).unwrap()
}

#[test]
fn a_repeated_picture_is_a_repeat_until_one_block_of_its_luma_changes() {
    let first = picture();
    let mut repeats = Repeats::default();
    assert!(
        repeats.needs_screen(&yuv(&first)),
        "the first sample is screened"
    );
    assert!(!repeats.needs_screen(&yuv(&first)));
    let mut noisy = first.clone();
    for (i, sample) in noisy[..64 * 36].iter_mut().enumerate() {
        *sample = if i % 2 == 0 {
            sample.saturating_add(3)
        } else {
            sample.saturating_sub(3)
        };
    }
    assert!(
        !repeats.needs_screen(&yuv(&noisy)),
        "sensor-level noise repeats"
    );
    let mut chroma = first.clone();
    for sample in &mut chroma[64 * 36..] {
        *sample = 200;
    }
    assert!(
        !repeats.needs_screen(&yuv(&chroma)),
        "the check reads luma only"
    );
    let mut changed = first.clone();
    for y in 0..32 {
        for x in 32..64 {
            changed[y * 64 + x] = changed[y * 64 + x].wrapping_add(60);
        }
    }
    assert!(repeats.needs_screen(&yuv(&changed)), "one changed block");
    assert!(
        !repeats.needs_screen(&yuv(&changed)),
        "the changed picture is now the last screened one"
    );
    let mut glyph = changed.clone();
    for y in 33..36 {
        for x in 0..8 {
            glyph[y * 64 + x] = 255;
        }
    }
    assert!(
        repeats.needs_screen(&yuv(&glyph)),
        "a small glyph in a partial block"
    );
}

#[test]
fn a_slow_change_never_drifts_through_a_chain_of_repeats() {
    let mut repeats = Repeats::default();
    let first = picture();
    assert!(repeats.needs_screen(&yuv(&first)));
    let mut screened = 0;
    for step in 1..=10u8 {
        let mut faded = first.clone();
        for sample in &mut faded[..64 * 36] {
            *sample = sample.saturating_sub(step * 2);
        }
        if repeats.needs_screen(&yuv(&faded)) {
            screened += 1;
        }
    }
    assert!(screened >= 2, "the fade is screened again: {screened}");
}

#[test]
fn a_padded_picture_keeps_the_frame_on_top_and_black_rows_below() {
    let bytes = picture();
    let colour = Coefficients::new(Matrix::Bt709, Range::Full);
    let frame = padded(&yuv(&bytes), &colour);
    assert_eq!(
        (frame.width, frame.height, frame.padded_height),
        (64, 36, 64)
    );
    assert_eq!(frame.rgb.len(), 64 * 64 * 3);
    assert_eq!(
        &frame.rgb[..3],
        &[bytes[0]; 3],
        "full range grey stays grey"
    );
    assert!(frame.rgb[64 * 36 * 3..].iter().all(|&value| value == 0));
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
