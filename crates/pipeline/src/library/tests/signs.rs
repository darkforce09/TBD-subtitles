use std::path::PathBuf;

use job_model::onscreen::{PixelRect, Plate, ReplacedText, TextCheck};

use super::*;
use crate::library::fixtures::{Scratch, hash, library_sign, occurrence, style};

fn document(occurrences: Vec<TextOccurrence>) -> TextDocument {
    TextDocument {
        width: 1920,
        height: 1080,
        occurrences,
        ..TextDocument::default()
    }
}

/// `id`'s replacement with `status`, its one plate's patch and mask written under `root`.
fn replaced(root: &Path, id: &str, status: ReplaceStatus) -> ReplacedText {
    let patch = PathBuf::from(format!("visual/patches/{id}/patch.png"));
    let mask = PathBuf::from(format!("visual/masks/{id}/mask.png"));
    for (path, bytes) in [
        (&patch, b"patch bytes".as_slice()),
        (&mask, b"mask".as_slice()),
    ] {
        std::fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
        std::fs::write(root.join(path), bytes).unwrap();
    }
    ReplacedText {
        id: id.into(),
        first_frame: 24,
        last_frame: 72,
        status,
        style: Some(style()),
        container: None,
        plates: vec![Plate {
            first_frame: 24,
            last_frame: 72,
            rect: PixelRect {
                x: 0,
                y: 0,
                width: 128,
                height: 64,
            },
            shift: [0.0, 0.0],
            scale: 1.0,
            source: PathBuf::from(format!("visual/masks/{id}/source.png")),
            mask,
            plate: None,
            patch: Some(patch),
            shifted: Vec::new(),
        }],
        preview: None,
        lettering_quad: None,
    }
}

fn check(id: &str, passed: &[bool]) -> TextCheck {
    TextCheck {
        id: id.into(),
        samples: passed.len() as u32,
        passed: passed.iter().all(|passed| *passed),
    }
}

#[test]
fn occurrences_match_signs_of_other_jobs_by_text_and_crop() {
    let scratch = Scratch::new("matches");
    let root = scratch.path().join("job");
    let library = Library::at(scratch.path().join(super::super::FILE_NAME));
    library
        .record_all(vec![
            library_sign("王宮", hash(1), "Royal Palace", "d11"),
            library_sign("港", hash(2), "Harbor", "d28"),
        ])
        .unwrap();
    let text = document(vec![
        occurrence(&root, "a", "王 宮", 1),
        occurrence(&root, "b", "王宮", 9),
        occurrence(&root, "c", "港", 2),
        occurrence(&root, "d", "", 1),
    ]);
    let found = matches(&library, &text, &root, "d28").unwrap();
    assert_eq!(found.keys().collect::<Vec<_>>(), vec!["a"]);
    assert_eq!(found["a"].english, "Royal Palace");
    let from_d12 = matches(&library, &text, &root, "d12").unwrap();
    assert_eq!(from_d12.keys().collect::<Vec<_>>(), vec!["a", "c"]);
}

#[test]
fn an_empty_library_matches_nothing_and_its_digest_is_none() {
    let scratch = Scratch::new("no-matches");
    let root = scratch.path().join("job");
    let library = Library::at(scratch.path().join(super::super::FILE_NAME));
    let text = document(vec![occurrence(&root, "a", "王宮", 1)]);
    let found = matches(&library, &text, &root, "d11").unwrap();
    assert!(found.is_empty());
    assert_eq!(digest(&found), None);
    assert!(!library.path().exists());
}

#[test]
fn the_digest_follows_the_english_the_style_and_the_key_of_each_match() {
    let mut matched = Matches::new();
    matched.insert("a".into(), library_sign("王宮", 7, "Royal Palace", "d11"));
    let base = digest(&matched).unwrap();
    assert_eq!(digest(&matched.clone()).unwrap(), base);
    let changed = |change: &dyn Fn(&mut LibrarySign)| {
        let mut other = matched.clone();
        change(other.get_mut("a").unwrap());
        digest(&other).unwrap()
    };
    assert_ne!(changed(&|s| s.english = "The Palace".into()), base);
    assert_ne!(changed(&|s| s.style.fill_rgb = [0, 0, 0]), base);
    assert_ne!(changed(&|s| s.crop_hash = 8), base);
    assert_ne!(changed(&|s| s.japanese = "城".into()), base);
    assert_eq!(changed(&|s| s.episodes.push("d28".into())), base);
}

#[test]
fn a_finished_job_approves_only_baked_replacements_that_read_back() {
    let scratch = Scratch::new("approved");
    let root = scratch.path().to_path_buf();
    let mut kept_japanese = occurrence(&root, "kept", "海", 4);
    kept_japanese.reviewed = true;
    kept_japanese.presentation.treatment = TextTreatment::Nearby;
    let mut owner = occurrence(&root, "owner", "城", 5);
    owner.reviewed = true;
    owner.confidence = 0.4;
    let text = document(vec![
        occurrence(&root, "good", "王宮", 1),
        occurrence(&root, "failed", "港", 2),
        occurrence(&root, "fallback", "島", 3),
        kept_japanese,
        owner,
        occurrence(&root, "unchecked", "町", 6),
    ]);
    let verified = VerifiedReplacements {
        document: ReplacementDocument {
            width: 1920,
            height: 1080,
            frame_count: 100,
            texts: vec![
                replaced(&root, "good", ReplaceStatus::Baked),
                replaced(&root, "failed", ReplaceStatus::Baked),
                replaced(&root, "fallback", ReplaceStatus::Fallback("left".into())),
                replaced(&root, "kept", ReplaceStatus::Baked),
                replaced(&root, "owner", ReplaceStatus::Baked),
                replaced(&root, "unchecked", ReplaceStatus::Baked),
            ],
        },
        checks: vec![
            check("good", &[true, true]),
            check("failed", &[true, false]),
            check("fallback", &[true]),
            check("kept", &[true]),
            check("owner", &[true]),
            check("unchecked", &[]),
        ],
    };
    let signs = approved(&verified, &text, &root, "d11", 42).unwrap();
    let ids: Vec<&str> = signs.iter().map(|s| s.japanese.as_str()).collect();
    assert_eq!(ids, vec!["王宮", "城"]);
    assert_eq!(signs[0].english, "English of good");
    assert_eq!(signs[0].crop_hash, hash(1));
    assert_eq!(signs[0].confidence, 0.9);
    assert_eq!(signs[0].patch_png, b"patch bytes");
    assert_eq!(signs[0].mask_png, b"mask");
    assert_eq!(signs[0].episodes, vec!["d11".to_string()]);
    assert_eq!(signs[0].added_s, 42);
    assert_eq!(signs[1].confidence, 1.0);
}

#[test]
fn a_rejected_occurrence_forgets_its_sign() {
    let scratch = Scratch::new("forget");
    let root = scratch.path().join("job");
    let library = Library::at(scratch.path().join(super::super::FILE_NAME));
    library
        .record(library_sign("王宮", hash(1), "Royal Palace", "d11"))
        .unwrap();
    let other = occurrence(&root, "b", "王宮", 9);
    assert_eq!(forget(&library, &other, &root).unwrap(), 0);
    let rejected = occurrence(&root, "a", "王宮", 1);
    assert_eq!(forget(&library, &rejected, &root).unwrap(), 1);
    assert_eq!(library.size().unwrap().signs, 0);
}

#[test]
fn the_composition_starts_pending_matches_from_the_stored_style() {
    let scratch = Scratch::new("styles");
    let root = scratch.path().to_path_buf();
    let mut document = ReplacementDocument {
        width: 1920,
        height: 1080,
        frame_count: 100,
        texts: vec![
            replaced(&root, "a", ReplaceStatus::Pending),
            replaced(&root, "b", ReplaceStatus::Pending),
            replaced(&root, "c", ReplaceStatus::Fallback("left".into())),
        ],
    };
    for text in &mut document.texts {
        text.style = None;
    }
    let mut known = Matches::new();
    known.insert("a".into(), library_sign("王宮", 1, "Royal Palace", "d11"));
    known.insert("c".into(), library_sign("港", 2, "Harbor", "d11"));
    start_from_styles(&mut document, &known);
    assert_eq!(document.texts[0].style, Some(style()));
    assert_eq!(document.texts[1].style, None);
    assert_eq!(document.texts[2].style, None);
}
