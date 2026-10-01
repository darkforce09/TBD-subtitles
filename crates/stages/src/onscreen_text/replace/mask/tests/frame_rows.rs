//! The per-frame rows of mask extraction: one per frame of every occurrence that keeps its
//! plates, following the writing, and the plate splits they imply.

use std::path::Path;

use image::GrayImage;
use job_model::onscreen::{
    FrameRecord, Quad, ReplaceStatus, ReplacementDocument, TextDocument, decode_mask, mask_area,
};

use super::extract;
use super::fixtures::{Background, FPS, Glyphs, Scripted, job_dir, occurrence, still_occurrence};
use super::runs::MIN_MASK_IOU;

const SIZE: (u32, u32) = (640, 360);

/// Every frame row an extraction sent, in order.
pub(crate) type Rows = Vec<(String, u64, FrameRecord)>;

fn document(occurrences: Vec<job_model::onscreen::TextOccurrence>) -> TextDocument {
    TextDocument {
        width: SIZE.0,
        height: SIZE.1,
        occurrences,
        ..TextDocument::default()
    }
}

/// Extract `text`, check the document and its rows against each other, and return both.
pub(crate) fn run_rows(
    text: &TextDocument,
    source: &mut Scripted,
    root: &Path,
) -> (ReplacementDocument, Rows) {
    let mut rows = Rows::new();
    let document = extract(
        text,
        source,
        root,
        &mut |id, frame, record| {
            rows.push((id.to_string(), frame, record.clone()));
            Ok(())
        },
        &|_, _| {},
    )
    .expect("extraction succeeds");
    document.validate().expect("the document validates");
    check_rows(&document, &rows, root);
    (document, rows)
}

/// Each occurrence with plates has exactly one row per frame of its span, in order; each row's
/// plate covers its frame and holds its mask; each plate's mask file is the union of its frames'
/// masks, which each cover at least `MIN_MASK_IOU` of it; occurrences without plates have none.
fn check_rows(document: &ReplacementDocument, rows: &Rows, root: &Path) {
    let mut at = 0;
    for text in &document.texts {
        if text.plates.is_empty() {
            assert!(rows.iter().all(|(id, _, _)| *id != text.id), "{}", text.id);
            continue;
        }
        let mut unions: Vec<Vec<u8>> = text
            .plates
            .iter()
            .map(|p| vec![0; (p.rect.width * p.rect.height) as usize])
            .collect();
        for frame in text.first_frame..=text.last_frame {
            let (id, number, record) = &rows[at];
            at += 1;
            assert_eq!((id.as_str(), *number), (text.id.as_str(), frame));
            let plate = &text.plates[record.plate as usize];
            assert!((plate.first_frame..=plate.last_frame).contains(&frame));
            assert_eq!(record.scale, plate.scale);
            let mask = decode_mask(plate.rect.width, plate.rect.height, &record.mask);
            for (union, value) in unions[record.plate as usize].iter_mut().zip(&mask) {
                *union |= *value;
            }
        }
        for (plate, union) in text.plates.iter().zip(&unions) {
            let file = image::open(root.join(&plate.mask))
                .expect("mask")
                .to_luma8();
            let on: Vec<u8> = file
                .as_raw()
                .iter()
                .map(|&v| u8::from(v != 0) * 255)
                .collect();
            assert_eq!(
                &on, union,
                "{}: the plate mask is its frames' union",
                text.id
            );
        }
        for (_, _, record) in rows.iter().filter(|(id, _, _)| *id == text.id) {
            let plate = &text.plates[record.plate as usize];
            let union = unions[record.plate as usize]
                .iter()
                .filter(|&&v| v != 0)
                .count();
            let share = mask_area(&record.mask) as f64 / union.max(1) as f64;
            assert!(
                share >= MIN_MASK_IOU - 1e-9,
                "{}: {share} of {:?}",
                text.id,
                plate.rect
            );
        }
    }
    assert_eq!(at, rows.len(), "no row outside an occurrence's span");
}

fn moving(
    id: &str,
    glyphs: &Glyphs,
    step: impl Fn(u64) -> i64 + 'static,
    frames: u64,
) -> (Scripted, TextDocument) {
    let paint = glyphs.clone();
    let shift = std::sync::Arc::new(step);
    let painted = shift.clone();
    let source = Scripted::new(frames as usize, SIZE, move |index, x, y| {
        paint
            .shifted(painted(index), 0)
            .paint(x, y, Background::Flat.at(x, y))
    });
    let samples: Vec<(f64, f64, Quad)> = (0..frames)
        .map(|f| {
            let quad = glyphs.shifted(shift(f), 0).quad(4.0);
            (f as f64 / FPS, (f + 1) as f64 / FPS, quad)
        })
        .collect();
    let key = frames / 2;
    let item = occurrence(id, &samples, (key as f64 + 0.5) / FPS);
    (source, document(vec![item]))
}

#[test]
fn a_still_sign_keeps_one_plate_and_every_frame_row_holds_its_mask() {
    let root = job_dir("rows-still");
    let glyphs = Glyphs::row(200, 160, 2);
    let paint = glyphs.clone();
    let mut source = Scripted::new(10, SIZE, move |_, x, y| {
        paint.paint(x, y, Background::Gradient.at(x, y))
    });
    let text = document(vec![still_occurrence("s", glyphs.quad(4.0), 0, 9)]);
    let (result, rows) = run_rows(&text, &mut source, &root);
    let plates = &result.texts[0].plates;
    assert_eq!(plates.len(), 1);
    assert!(plates[0].mask.ends_with("mask.png"));
    assert_eq!(rows.len(), 10);
    let key: GrayImage = image::open(root.join(&plates[0].mask)).unwrap().to_luma8();
    for (_, _, record) in &rows {
        assert_eq!(
            (record.shift, record.scale, record.plate),
            ([0.0, 0.0], 1.0, 0)
        );
        assert!(record.follow_score > 0.99, "{}", record.follow_score);
        let mask = decode_mask(key.width(), key.height(), &record.mask);
        let file: Vec<u8> = key
            .as_raw()
            .iter()
            .map(|&v| u8::from(v != 0) * 255)
            .collect();
        assert_eq!(mask, file);
    }
    assert!(rows.iter().all(|(_, _, r)| r.quad == rows[0].2.quad));
}

#[test]
fn moving_writing_splits_plates_and_its_rows_follow_the_shift() {
    let root = job_dir("rows-moving");
    let glyphs = Glyphs::row(200, 160, 2);
    let (mut source, text) = moving("m", &glyphs, |f| 3 * f as i64, 12);
    let (result, rows) = run_rows(&text, &mut source, &root);
    let item = &result.texts[0];
    assert_eq!(item.status, ReplaceStatus::Pending);
    assert!(item.plates.len() > 1, "the erase follows the writing");
    let key = rows[6].2.quad;
    for (_, frame, record) in &rows {
        let expected = 3.0 * (*frame as f64 - 6.0);
        assert_eq!(record.shift, [expected, 0.0], "frame {frame}");
        assert!(record.follow_score >= 0.8, "frame {frame}");
        let moved = |q: Quad| q.0.map(|p| (p.x, p.y));
        let carried = moved(key).map(|(x, y)| (x + expected, y));
        assert_eq!(moved(record.quad), carried, "frame {frame}");
    }
    for plate in &item.plates {
        let inside: Vec<&FrameRecord> = rows
            .iter()
            .filter(|(_, f, _)| (plate.first_frame..=plate.last_frame).contains(f))
            .map(|(_, _, r)| r)
            .collect();
        assert_eq!(
            plate.shift, inside[0].shift,
            "a plate starts at its first frame's shift"
        );
    }
}

#[test]
fn slowly_drifting_writing_shares_plates_whose_erase_covers_every_place() {
    let root = job_dir("rows-drift");
    let glyphs = Glyphs::row(200, 160, 2);
    let (mut source, text) = moving("d", &glyphs, |f| (f / 2) as i64, 16);
    let (result, rows) = run_rows(&text, &mut source, &root);
    let item = &result.texts[0];
    assert_eq!(item.status, ReplaceStatus::Pending);
    for (_, frame, record) in &rows {
        assert_eq!(record.shift[0], (frame / 2) as f64 - 4.0, "frame {frame}");
    }
    assert!(
        item.plates.len() < 8,
        "one-pixel steps share plates: {:?}",
        item.plates
    );
    let shared = item.plates.iter().enumerate().find(|(index, _)| {
        let mut shifts: Vec<f64> = rows
            .iter()
            .filter(|(_, _, r)| r.plate as usize == *index)
            .map(|(_, _, r)| r.shift[0])
            .collect();
        shifts.dedup();
        shifts.len() > 1
    });
    let (index, plate) = shared.expect("a plate holds frames at two shifts");
    let erase = image::open(root.join(&plate.mask)).unwrap().to_luma8();
    let erased = erase.pixels().filter(|p| p.0[0] != 0).count() as u64;
    let own = rows
        .iter()
        .find(|(_, _, r)| r.plate as usize == index)
        .map(|(_, _, r)| mask_area(&r.mask))
        .unwrap();
    assert!(
        erased > own,
        "the plate erases every place its frames show the writing"
    );
}
