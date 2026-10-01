use std::collections::BTreeSet;

use super::*;

/// An IDR check over a fixed set of IDR frames that records every batch it is asked.
struct FakeIdr {
    idr: BTreeSet<u64>,
    asked: Vec<Vec<u64>>,
}

impl FakeIdr {
    fn new(idr: &[u64]) -> FakeIdr {
        FakeIdr {
            idr: idr.iter().copied().collect(),
            asked: Vec::new(),
        }
    }

    fn plan(
        &mut self,
        changed: &[FrameSpan],
        keyframes: &[u64],
        frames: u64,
    ) -> Result<Vec<Piece>, PlanError> {
        plan_pieces(changed, keyframes, frames, &mut |batch| {
            self.asked.push(batch.to_vec());
            Ok(batch.iter().map(|frame| self.idr.contains(frame)).collect())
        })
    }

    fn all_asked(&self) -> Vec<u64> {
        self.asked.iter().flatten().copied().collect()
    }
}

fn span(first: u64, last: u64) -> FrameSpan {
    FrameSpan { first, last }
}

fn copy(first: u64, last: u64) -> Piece {
    Piece::Copy { first, last }
}

fn encode(first: u64, last: u64) -> Piece {
    Piece::Encode { first, last }
}

/// Keyframes every 24 frames of a 240-frame video, all IDR.
const KEYS: [u64; 10] = [0, 24, 48, 72, 96, 120, 144, 168, 192, 216];

#[test]
fn no_changed_frames_copy_the_whole_video_without_a_check() {
    let mut idr = FakeIdr::new(&KEYS);
    assert_eq!(idr.plan(&[], &KEYS, 240).unwrap(), [copy(0, 239)]);
    assert!(idr.asked.is_empty());
}

#[test]
fn a_change_inside_a_gop_widens_to_the_idr_keyframes_around_it() {
    let mut idr = FakeIdr::new(&KEYS);
    let pieces = idr.plan(&[span(30, 40)], &KEYS, 240).unwrap();
    assert_eq!(pieces, [copy(0, 23), encode(24, 47), copy(48, 239)]);
    assert_eq!(idr.asked, [vec![24, 48]]);
}

#[test]
fn a_change_on_a_keyframe_starts_there_and_one_ending_on_a_keyframe_includes_its_gop() {
    let mut idr = FakeIdr::new(&KEYS);
    assert_eq!(
        idr.plan(&[span(48, 71)], &KEYS, 240).unwrap(),
        [copy(0, 47), encode(48, 71), copy(72, 239)]
    );
    assert_eq!(
        idr.plan(&[span(50, 72)], &KEYS, 240).unwrap(),
        [copy(0, 47), encode(48, 95), copy(96, 239)]
    );
}

#[test]
fn keyframes_that_are_not_idr_widen_outward_to_the_next_idr() {
    // 48 and 72 are open-GOP keyframes; 24 and 96 are IDR.
    let mut idr = FakeIdr::new(&[0, 24, 96, 120, 144, 168, 192, 216]);
    let pieces = idr.plan(&[span(50, 60)], &KEYS, 240).unwrap();
    assert_eq!(pieces, [copy(0, 23), encode(24, 95), copy(96, 239)]);
    assert_eq!(idr.asked, [vec![48, 72], vec![24, 96]]);
}

#[test]
fn without_an_idr_before_or_after_the_piece_reaches_the_ends() {
    let mut idr = FakeIdr::new(&[0]);
    let pieces = idr.plan(&[span(100, 110)], &KEYS, 240).unwrap();
    assert_eq!(pieces, [encode(0, 239)]);
    assert!(!idr.all_asked().contains(&0), "frame 0 needs no check");
    let mut idr = FakeIdr::new(&KEYS);
    assert_eq!(
        idr.plan(&[span(220, 239)], &KEYS, 240).unwrap(),
        [copy(0, 215), encode(216, 239)]
    );
    assert_eq!(
        idr.plan(&[span(0, 3)], &KEYS, 240).unwrap(),
        [encode(0, 23), copy(24, 239)]
    );
}

#[test]
fn a_video_whose_first_frame_is_no_keyframe_encodes_from_its_start() {
    let keys = [24, 48];
    let mut idr = FakeIdr::new(&keys);
    assert_eq!(
        idr.plan(&[span(10, 12)], &keys, 72).unwrap(),
        [encode(0, 23), copy(24, 71)]
    );
}

#[test]
fn overlapping_adjacent_and_unordered_changes_merge() {
    let mut idr = FakeIdr::new(&KEYS);
    let changed = [span(150, 160), span(30, 35), span(36, 40), span(33, 50)];
    let pieces = idr.plan(&changed, &KEYS, 240).unwrap();
    assert_eq!(
        pieces,
        [
            copy(0, 23),
            encode(24, 71),
            copy(72, 143),
            encode(144, 167),
            copy(168, 239)
        ]
    );
}

#[test]
fn widened_pieces_that_meet_become_one() {
    let mut idr = FakeIdr::new(&KEYS);
    // 30..=40 widens to 24..48 and 50..=60 to 48..72: they meet at 48.
    let pieces = idr.plan(&[span(30, 40), span(50, 60)], &KEYS, 240).unwrap();
    assert_eq!(pieces, [copy(0, 23), encode(24, 71), copy(72, 239)]);
}

#[test]
fn each_keyframe_is_asked_about_once_in_ascending_batches() {
    let mut idr = FakeIdr::new(&[0, 24, 120]);
    idr.plan(&[span(30, 40), span(100, 110)], &KEYS, 240)
        .unwrap();
    for batch in &idr.asked {
        assert!(batch.windows(2).all(|pair| pair[0] < pair[1]), "{batch:?}");
    }
    let asked = idr.all_asked();
    let unique: BTreeSet<u64> = asked.iter().copied().collect();
    assert_eq!(asked.len(), unique.len(), "{asked:?}");
}

#[test]
fn changes_or_keyframes_outside_the_video_are_refused() {
    let mut idr = FakeIdr::new(&KEYS);
    let refused = [
        idr.plan(&[span(10, 240)], &KEYS, 240),
        idr.plan(&[span(20, 10)], &KEYS, 240),
        idr.plan(&[], &KEYS, 0),
        idr.plan(&[span(1, 2)], &[0, 48, 24], 240),
        idr.plan(&[span(1, 2)], &[0, 24, 24], 240),
        idr.plan(&[span(1, 2)], &[0, 240], 240),
    ];
    for result in refused {
        assert!(matches!(result, Err(PlanError::Invalid(_))), "{result:?}");
    }
}

#[test]
fn a_failed_or_short_check_is_a_probe_error() {
    let failed = plan_pieces(&[span(30, 40)], &KEYS, 240, &mut |_| {
        Err(MediaError::Parse("no ffprobe".into()))
    });
    assert!(matches!(failed, Err(PlanError::Probe(_))));
    let short = plan_pieces(&[span(30, 40)], &KEYS, 240, &mut |_| Ok(vec![true]));
    assert!(matches!(short, Err(PlanError::Probe(_))));
    assert!(failed.unwrap_err().to_string().contains("no ffprobe"));
}

/// A small deterministic generator for the coverage sweep.
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, bound: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % bound
    }
}

#[test]
fn every_plan_covers_each_frame_once_and_starts_every_piece_where_a_decoder_can() {
    let mut random = Lcg(7);
    for _ in 0..500 {
        let frames = 1 + random.below(300);
        let keyframes: Vec<u64> = (0..frames).filter(|_| random.below(10) == 0).collect();
        let idr: Vec<u64> = keyframes
            .iter()
            .copied()
            .filter(|_| random.below(3) != 0)
            .collect();
        let changed: Vec<FrameSpan> = (0..random.below(5))
            .map(|_| {
                let first = random.below(frames);
                span(first, (first + random.below(40)).min(frames - 1))
            })
            .collect();
        let mut fake = FakeIdr::new(&idr);
        let pieces = fake.plan(&changed, &keyframes, frames).unwrap();
        let mut next = 0;
        for (number, piece) in pieces.iter().enumerate() {
            assert_eq!(piece.first(), next, "{pieces:?}");
            assert!(piece.last() >= piece.first());
            assert!(
                piece.first() == 0 || idr.contains(&piece.first()),
                "piece {number} of {pieces:?} starts on no IDR ({idr:?})"
            );
            if number > 0 {
                assert_ne!(piece.is_copy(), pieces[number - 1].is_copy(), "{pieces:?}");
            }
            next = piece.last() + 1;
        }
        assert_eq!(next, frames);
        for change in &changed {
            for frame in change.first..=change.last {
                assert!(
                    pieces
                        .iter()
                        .any(|piece| !piece.is_copy()
                            && (piece.first()..=piece.last()).contains(&frame)),
                    "frame {frame} is copied in {pieces:?}"
                );
            }
        }
    }
}

#[test]
fn the_summary_counts_segments_and_frames() {
    let summary = piece_summary(&[copy(0, 23), encode(24, 71), copy(72, 143), encode(144, 167)]);
    assert_eq!(summary.segments_reencoded, 2);
    assert_eq!(summary.frames_reencoded, 72);
    assert_eq!(summary.frames_copied, 96);
    assert_eq!(summary.fallback_reason, None);
    assert_eq!(copy(5, 5).frames(), 1);
}
