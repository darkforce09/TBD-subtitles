use job_model::outputs::ShotChanges;
use job_model::outputs::ShotCut;

use super::*;
use crate::onscreen_text::detect::fixtures::{timeline, yuv_frame};
use crate::onscreen_text::detect::screen::Samples;

fn picture() -> PaddedFrame {
    PaddedFrame {
        width: 2,
        height: 2,
        padded_height: 32,
        rgb: vec![0; 2 * 32 * 3],
    }
}

#[test]
fn a_group_closes_on_a_full_batch_or_twice_a_batch_of_samples_and_holds_every_frame_once() {
    for (k, screen_every) in [(1u64, 1usize), (5, 1), (12, 3), (25, 1), (12, 100)] {
        let frames = timeline(400);
        let cuts = ShotChanges {
            cuts: vec![ShotCut {
                time_s: frames[133].0,
                score: 50.0,
            }],
        };
        let samples = Samples::new(&frames, &cuts, k);
        let batch = 4;
        let bound = (2 * batch + 1) * k as usize;
        let mut gathering = Gathering::default();
        let (mut held, mut seen) = (0, 0);
        for index in 0..400 {
            let frame = yuv_frame(&[], index, false, &frames);
            if samples.contains(index) {
                seen += 1;
                let screened = seen % screen_every == 0 || seen == 1;
                gathering.push_sample(frame, screened.then(picture));
                if gathering.full(batch) {
                    let (group, pictures) = gathering.close();
                    assert!(pictures.len() <= batch);
                    assert!(group.len() <= 2 * batch);
                    assert_eq!(
                        pictures.len(),
                        group.iter().filter(|sample| sample.screened).count()
                    );
                    for sample in &group {
                        assert!(sample.gap.len() < k as usize, "a gap holds at most k - 1");
                        if sample.frame.index > 0 {
                            assert_eq!(
                                sample.previous_sample() + sample.gap.len() as u64 + 1,
                                sample.frame.index
                            );
                        }
                        for frame in &sample.gap {
                            assert_eq!(sample.frame(frame.index).unwrap().index, frame.index);
                        }
                        held += 1 + sample.gap.len();
                    }
                }
            } else {
                gathering.push_gap(frame);
            }
            assert!(
                gathering.frames() <= bound,
                "k {k}: {} frames",
                gathering.frames()
            );
        }
        held += gathering.frames();
        let (rest, _) = gathering.finish().unwrap();
        assert!(rest.len() < 2 * batch);
        assert_eq!(held, 400, "every frame passes through exactly one group");
    }
}

#[test]
fn a_stream_that_stops_between_samples_is_an_error() {
    let frames = timeline(2);
    let mut gathering = Gathering::default();
    gathering.push_sample(yuv_frame(&[], 0, false, &frames), Some(picture()));
    gathering.push_gap(yuv_frame(&[], 1, false, &frames));
    assert!(gathering.finish().is_err());
}

#[test]
fn a_group_finds_any_frame_it_holds() {
    let frames = timeline(30);
    let mut gathering = Gathering::default();
    for index in 0..25 {
        let frame = yuv_frame(&[], index, false, &frames);
        if index % 12 == 0 {
            gathering.push_sample(frame, None);
        } else {
            gathering.push_gap(frame);
        }
    }
    let (samples, pictures) = gathering.close();
    assert!(pictures.is_empty());
    let group = Group {
        prior: Vec::new(),
        samples,
        job: None,
    };
    for index in 0..25 {
        assert_eq!(group.frame(index).unwrap().index, index);
    }
    assert!(group.frame(25).is_none());
}

#[test]
fn a_group_finds_frames_in_its_carried_samples_and_their_gaps() {
    let frames = timeline(24);
    let prior_frame = yuv_frame(&[], 11, false, &frames);
    let gap: Vec<_> = (0..11).map(|i| yuv_frame(&[], i, false, &frames)).collect();
    let prior = HeldSample {
        frame: std::sync::Arc::new(prior_frame),
        gap,
        screened: true,
    };
    let group = Group {
        prior: vec![prior],
        samples: Vec::new(),
        job: None,
    };
    for index in 0..=11 {
        assert_eq!(group.frame(index).unwrap().index, index);
    }
    assert!(group.frame(12).is_none());
}
