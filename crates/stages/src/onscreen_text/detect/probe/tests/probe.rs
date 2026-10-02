use std::sync::Arc;

use media_io::yuv::{Matrix, Range};

use super::*;
use crate::onscreen_text::detect::fixtures::{region, timeline, writing, yuv_frame};
use crate::onscreen_text::detect::window::HeldSample;

/// One group: the sample at frame 24 with the frames since sample 12, and sample 36 with its gap.
fn group(writings: &[crate::onscreen_text::detect::fixtures::Writing]) -> Group {
    let frames = timeline(48);
    let sample = |index: u64| HeldSample {
        frame: Arc::new(yuv_frame(writings, index, false, &frames)),
        gap: (index - 11..index)
            .map(|gap| yuv_frame(writings, gap, false, &frames))
            .collect(),
        screened: true,
    };
    Group {
        prior: None,
        samples: vec![sample(24), sample(36)],
        job: None,
    }
}

fn colour() -> Coefficients {
    Coefficients::new(Matrix::Bt709, Range::Full)
}

#[test]
fn an_entry_and_an_exit_narrow_to_their_exact_frames_through_one_screen_per_step() {
    let writings = [writing(17..=30, 20, 240)];
    let group = group(&writings);
    let colour = colour();
    let anchor_box = region(&writings[0], 0.0);
    let shown = group.samples[0].frame(20).unwrap().picture().unwrap();
    let signature = picture_at(&shown, &colour, anchor_box);
    let transitions = [
        Transition {
            occurrence: 0,
            search: Search::new(12, 24, Seek::Entry),
            quad: anchor_box,
            anchor_box,
            signature: signature.clone(),
        },
        Transition {
            occurrence: 0,
            search: Search::new(24, 36, Seek::Exit),
            quad: anchor_box,
            anchor_box,
            signature,
        },
    ];
    let mut calls: Vec<usize> = Vec::new();
    let mut screen = |pictures: Vec<PaddedFrame>| -> TextResult<Vec<Vec<(Quad, f64)>>> {
        calls.push(pictures.len());
        Ok(pictures
            .iter()
            .map(|picture| {
                let at = (12 * picture.width as usize + 16) * 3;
                if picture.rgb[at] == 20 {
                    vec![(anchor_box, 0.9)]
                } else {
                    Vec::new()
                }
            })
            .collect())
    };
    let mut stats = ScanStats::default();
    let answers = narrow(&transitions, &group, &colour, &mut screen, &mut stats).unwrap();
    assert_eq!(answers, [17, 31]);
    assert_eq!(
        calls,
        [2, 1],
        "signature match fast-rejects absent frames from detector screening"
    );
    assert_eq!(stats.frames_probed, 3);
    assert_eq!(stats.frames_screened, 3);
}

#[test]
fn a_changed_picture_under_an_overlapping_box_is_absent() {
    let first = writing(10..=30, 20, 240);
    let second = writing(31..=40, 240, 20);
    let writings = [first.clone(), second];
    let group = group(&writings);
    let colour = colour();
    let anchor_box = region(&first, 0.0);
    let shown = group.samples[1].frame(26).unwrap().picture().unwrap();
    let transitions = [Transition {
        occurrence: 0,
        search: Search::new(24, 36, Seek::Exit),
        quad: anchor_box,
        anchor_box,
        signature: picture_at(&shown, &colour, anchor_box),
    }];
    // The detector boxes the second writing in the same place: only the anchor tells it apart.
    let mut screen = |pictures: Vec<PaddedFrame>| -> TextResult<Vec<Vec<(Quad, f64)>>> {
        Ok(pictures.iter().map(|_| vec![(anchor_box, 0.9)]).collect())
    };
    let answers = narrow(
        &transitions,
        &group,
        &colour,
        &mut screen,
        &mut ScanStats::default(),
    )
    .unwrap();
    assert_eq!(answers, [31]);
}

#[test]
fn fast_reject_skips_gpu_screening_when_all_active_signatures_reject() {
    let first = writing(10..=30, 20, 240);
    let writings = [first.clone()];
    let group = group(&writings);
    let colour = colour();
    let anchor_box = region(&first, 0.0);
    let shown = group.samples[0].frame(20).unwrap().picture().unwrap();
    let signature = picture_at(&shown, &colour, anchor_box);

    let transitions = [
        Transition {
            occurrence: 0,
            search: Search::new(30, 36, Seek::Exit),
            quad: anchor_box,
            anchor_box,
            signature: signature.clone(),
        },
        Transition {
            occurrence: 1,
            search: Search::new(30, 36, Seek::Exit),
            quad: anchor_box,
            anchor_box,
            signature: GrayImage::new(10, 10),
        },
    ];
    let mut screened = false;
    let mut screen = |pictures: Vec<PaddedFrame>| -> TextResult<Vec<Vec<(Quad, f64)>>> {
        screened = true;
        Ok(vec![vec![(anchor_box, 0.9)]; pictures.len()])
    };
    let mut stats = ScanStats::default();
    let answers = narrow(&transitions, &group, &colour, &mut screen, &mut stats).unwrap();
    assert_eq!(answers, [31, 31]);
    assert!(
        !screened,
        "all active searches rejected, so GPU screening was skipped"
    );
    assert_eq!(stats.frames_screened, 0);
    assert_eq!(stats.frames_probed, 0);
}

#[test]
fn a_probe_outside_the_held_frames_is_an_error() {
    let group = group(&[]);
    let colour = colour();
    let quad = region(&writing(0..=0, 20, 240), 0.0);
    let transitions = [Transition {
        occurrence: 0,
        search: Search::new(0, 12, Seek::Entry),
        quad,
        anchor_box: quad,
        signature: GrayImage::new(1, 1),
    }];
    let mut screen = |pictures: Vec<PaddedFrame>| -> TextResult<Vec<Vec<(Quad, f64)>>> {
        Ok(vec![Vec::new(); pictures.len()])
    };
    assert!(
        narrow(
            &transitions,
            &group,
            &colour,
            &mut screen,
            &mut ScanStats::default()
        )
        .is_err()
    );
}

#[test]
fn memoized_signatures_match_and_narrow_multiple_transitions_in_parallel() {
    let writings = [writing(15..=25, 20, 240), writing(20..=32, 20, 240)];
    let group = group(&writings);
    let colour = colour();
    let anchor_1 = region(&writings[0], 0.0);
    let anchor_2 = region(&writings[1], 0.0);
    let shown_1 = group.samples[0].frame(20).unwrap().picture().unwrap();
    let shown_2 = group.samples[0].frame(24).unwrap().picture().unwrap();
    let sig_1 = picture_at(&shown_1, &colour, anchor_1);
    let sig_2 = picture_at(&shown_2, &colour, anchor_2);
    let transitions = [
        Transition {
            occurrence: 0,
            search: Search::new(12, 24, Seek::Entry),
            quad: anchor_1,
            anchor_box: anchor_1,
            signature: sig_1,
        },
        Transition {
            occurrence: 1,
            search: Search::new(24, 36, Seek::Exit),
            quad: anchor_2,
            anchor_box: anchor_2,
            signature: sig_2,
        },
    ];
    let mut screen = |pictures: Vec<PaddedFrame>| -> TextResult<Vec<Vec<(Quad, f64)>>> {
        Ok(pictures
            .iter()
            .map(|_| vec![(anchor_1, 0.9), (anchor_2, 0.9)])
            .collect())
    };
    let mut stats = ScanStats::default();
    let answers = narrow(&transitions, &group, &colour, &mut screen, &mut stats).unwrap();
    assert_eq!(answers, [15, 33]);
}
