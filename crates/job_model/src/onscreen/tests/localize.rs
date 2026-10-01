use super::*;

fn plate(first: u64, last: u64) -> Plate {
    Plate {
        first_frame: first,
        last_frame: last,
        rect: PixelRect {
            x: 10,
            y: 10,
            width: 20,
            height: 10,
        },
        shift: [0.0, 0.0],
        scale: 1.0,
        source: PathBuf::from("visual/masks/a/source-0.png"),
        mask: PathBuf::from("visual/masks/a/mask.png"),
        plate: None,
        patch: None,
        shifted: Vec::new(),
    }
}

fn document(plates: Vec<Plate>) -> ReplacementDocument {
    ReplacementDocument {
        width: 64,
        height: 36,
        frame_count: 100,
        texts: vec![ReplacedText {
            id: "a".into(),
            first_frame: 10,
            last_frame: 20,
            status: ReplaceStatus::Baked,
            style: None,
            container: None,
            plates,
            preview: None,
            lettering_quad: None,
        }],
    }
}

#[test]
fn ordered_plates_inside_their_span_and_frame_are_valid() {
    assert_eq!(
        document(vec![plate(10, 14), plate(15, 20)]).validate(),
        Ok(())
    );
}

#[test]
fn overlapping_out_of_span_or_misplaced_plates_are_refused() {
    assert!(
        document(vec![plate(10, 15), plate(15, 20)])
            .validate()
            .is_err()
    );
    assert!(document(vec![plate(9, 12)]).validate().is_err());
    assert!(document(vec![plate(18, 21)]).validate().is_err());
    let mut outside = plate(10, 12);
    outside.rect.x = 50;
    assert!(document(vec![outside]).validate().is_err());
    let mut scaled = plate(10, 12);
    scaled.scale = 0.0;
    assert!(document(vec![scaled]).validate().is_err());
    let mut late = document(vec![]);
    late.texts[0].last_frame = 100;
    assert!(late.validate().is_err());
}

#[test]
fn only_baked_texts_are_drawn_and_the_status_reads_back() {
    let mut doc = document(vec![]);
    doc.texts.push(ReplacedText {
        id: "b".into(),
        status: ReplaceStatus::Fallback("too small".into()),
        ..doc.texts[0].clone()
    });
    assert_eq!(
        doc.baked().map(|t| t.id.as_str()).collect::<Vec<_>>(),
        ["a"]
    );
    let json = serde_json::to_string(&doc).unwrap();
    assert!(json.contains(r#"{"kind":"fallback","reason":"too small"}"#));
    assert_eq!(
        serde_json::from_str::<ReplacementDocument>(&json).unwrap(),
        doc
    );
}

#[test]
fn rectangles_know_their_edges_and_overlaps() {
    let a = PixelRect {
        x: 0,
        y: 0,
        width: 10,
        height: 10,
    };
    let b = PixelRect { x: 9, ..a };
    let c = PixelRect { x: 10, ..a };
    assert!(a.overlaps(b) && !a.overlaps(c));
    assert!(a.inside(10, 10) && !b.inside(10, 10));
    assert_eq!(a.area(), 100);
}
