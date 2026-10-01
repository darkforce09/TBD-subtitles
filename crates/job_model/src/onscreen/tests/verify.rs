use super::*;
use crate::onscreen::{ReplaceStatus, ReplacedText};

fn reading(frame: u64, similarity: f64, passed: bool) -> VerifyReading {
    VerifyReading {
        frame,
        japanese_found: String::new(),
        english_read: "read".into(),
        similarity,
        passed,
    }
}

fn verified() -> VerifiedReplacements {
    VerifiedReplacements {
        document: ReplacementDocument {
            width: 64,
            height: 36,
            frame_count: 100,
            texts: vec![ReplacedText {
                id: "a".into(),
                first_frame: 10,
                last_frame: 20,
                status: ReplaceStatus::Fallback("The English does not read back cleanly".into()),
                style: None,
                container: None,
                plates: Vec::new(),
                preview: None,
                lettering_quad: None,
            }],
        },
        checks: vec![TextCheck {
            id: "a".into(),
            samples: 2,
            passed: false,
        }],
    }
}

#[test]
fn the_file_reads_back_whole_and_as_a_plain_replacement_document() {
    let verified = verified();
    let json = serde_json::to_string(&verified).unwrap();
    assert_eq!(
        serde_json::from_str::<VerifiedReplacements>(&json).unwrap(),
        verified
    );
    assert_eq!(
        serde_json::from_str::<ReplacementDocument>(&json).unwrap(),
        verified.document
    );
}

#[test]
fn a_composed_document_reads_as_verified_without_checks() {
    let json = serde_json::to_string(&verified().document).unwrap();
    let read: VerifiedReplacements = serde_json::from_str(&json).unwrap();
    assert!(read.checks.is_empty());
    assert_eq!(read.document, verified().document);
}

#[test]
fn a_check_is_found_by_its_occurrence() {
    let verified = verified();
    assert_eq!(verified.check("a").map(|check| check.samples), Some(2));
    assert!(verified.check("b").is_none());
}

#[test]
fn the_telling_reading_is_the_first_failure_else_the_weakest_match() {
    let failing = [reading(10, 0.9, true), reading(15, 0.4, false)];
    assert_eq!(telling(&failing).unwrap().frame, 15);
    let passing = [reading(1, 0.9, true), reading(2, 0.7, true)];
    assert_eq!(telling(&passing).unwrap().frame, 2);
    assert!(telling(&[]).is_none());
}
