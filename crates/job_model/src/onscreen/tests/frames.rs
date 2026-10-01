use super::*;
use crate::archive_round_trip::round_trip;
use crate::onscreen::Point;

fn quad() -> Quad {
    Quad([
        Point { x: 10.0, y: 20.0 },
        Point { x: 110.0, y: 20.0 },
        Point { x: 110.0, y: 60.0 },
        Point { x: 10.0, y: 60.0 },
    ])
}

/// A 7 × 4 mask with runs at both edges, a single pixel and an empty row.
fn mask() -> Vec<u8> {
    #[rustfmt::skip]
    let pixels = vec![
        255, 255, 0, 0, 0, 0, 255,
        0, 0, 0, 255, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0,
        9, 9, 9, 9, 9, 9, 9,
    ];
    pixels
}

#[test]
fn a_mask_encodes_into_runs_and_decodes_back() {
    let runs = encode_mask(7, 4, &mask()).unwrap();
    assert_eq!(
        runs,
        vec![
            RleRun {
                row: 0,
                start: 0,
                len: 2
            },
            RleRun {
                row: 0,
                start: 6,
                len: 1
            },
            RleRun {
                row: 1,
                start: 3,
                len: 1
            },
            RleRun {
                row: 3,
                start: 0,
                len: 7
            },
        ]
    );
    assert_eq!(mask_area(&runs), 11);
    let decoded = decode_mask(7, 4, &runs);
    let expected: Vec<u8> = mask()
        .iter()
        .map(|&v| if v == 0 { 0 } else { 255 })
        .collect();
    assert_eq!(decoded, expected);
}

#[test]
fn an_empty_mask_has_no_runs_and_a_wrong_size_none() {
    assert_eq!(encode_mask(3, 2, &[0; 6]), Some(Vec::new()));
    assert_eq!(encode_mask(3, 2, &[0; 5]), None);
    assert_eq!(encode_mask(70_000, 1, &[]), None);
}

#[test]
fn runs_outside_the_mask_are_clipped() {
    let runs = [
        RleRun {
            row: 0,
            start: 2,
            len: 5,
        },
        RleRun {
            row: 9,
            start: 0,
            len: 1,
        },
    ];
    assert_eq!(decode_mask(4, 1, &runs), vec![0, 0, 255, 255]);
}

#[test]
fn a_frame_record_round_trips() {
    let record = FrameRecord {
        quad: quad(),
        follow_score: 0.93,
        shift: [3.0, -2.0],
        scale: 1.05,
        mask: encode_mask(7, 4, &mask()).unwrap(),
        plate: 2,
    };
    round_trip(&record);
    round_trip(&FrameRecord::default());
    let json = serde_json::to_string(&record).unwrap();
    assert_eq!(serde_json::from_str::<FrameRecord>(&json).unwrap(), record);
}
