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
            sample: 0,
            search: Search::new(12, 24, Seek::Entry),
            quad: anchor_box,
            anchor_box,
            signature: signature.clone(),
        },
        Transition {
            occurrence: 0,
            sample: 1,
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
        [2, 2, 2, 1],
        "one call per bisection step, distinct probes"
    );
    assert_eq!(stats.frames_probed, 7);
    assert_eq!(stats.frames_screened, 7);
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
        sample: 1,
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
fn a_probe_outside_the_held_frames_is_an_error() {
    let group = group(&[]);
    let colour = colour();
    let quad = region(&writing(0..=0, 20, 240), 0.0);
    let transitions = [Transition {
        occurrence: 0,
        sample: 0,
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
