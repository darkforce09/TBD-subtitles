use super::super::keyframe_requests::KeyframeAnswer;
use super::super::tests::{
    K1, K2, Temporary, claude_backend, dialogue, frame_document, region, sighting,
};
use super::super::{LocalOpener, TextResult, TranslationInput, run, vision};
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use inference::llm::claude_cli::ClaudeCli;
use inference::llm::{Completion, LanguageModel, LlmError};
use job_model::onscreen::{LetteringStyle, TextCorrections, TextSettings};
use job_model::outputs::ShotChanges;
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

fn library_sign(japanese: &str, english: &str) -> LibrarySign {
    LibrarySign {
        japanese: japanese.into(),
        crop_hash: 7,
        english: english.into(),
        confidence: 0.97,
        style: LetteringStyle {
            fill_rgb: [255, 255, 255],
            outline_rgb: None,
            outline_px: 0.0,
            soft_outline: false,
            stroke_px: 3.0,
            line_height_px: 30.0,
        },
        patch_png: Vec::new(),
        mask_png: Vec::new(),
        episodes: vec!["dressrosa-11".into()],
        added_s: 0,
    }
}

/// Writes each keyframe still and crop the document names; each file's bytes name its path.
fn write_images(root: &Path, document: &TextDocument) {
    for item in &document.occurrences {
        let still = item.keyframe.iter().map(|keyframe| &keyframe.image);
        for path in item.crops.iter().chain(still) {
            let file = root.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, format!("png bytes of {}", path.display())).unwrap();
        }
    }
}

/// Translate `document` under `root` with `known`, Claude answering through `ask`; the local
/// model must stay closed.
fn translate_with(
    root: &Path,
    document: &mut TextDocument,
    known: &BTreeMap<String, LibrarySign>,
    ask: &vision::Ask<'_>,
) -> TextResult<()> {
    let dialogue = dialogue();
    let settings = TextSettings::new_job();
    let corrections = TextCorrections::default();
    let input = TranslationInput {
        root,
        dialogue: &dialogue,
        cuts: &ShotChanges::default(),
        glossary: &[],
        settings: &settings,
        corrections: &corrections,
        excluded_reference: None,
        parallel_calls: 2,
    };
    let mut closed =
        || -> TextResult<Box<dyn LanguageModel>> { Err("the local model must stay closed".into()) };
    let open: &mut LocalOpener<'_> = &mut closed;
    let claude = claude_backend(root, Arc::new(AtomicBool::new(false)));
    run(
        document,
        &input,
        known,
        open,
        Some(&claude),
        ask,
        &|_, _| {},
    )
}

/// An ask that answers every request with `answer` and counts the requests and their images.
fn answering<'a>(
    answer: &'a KeyframeAnswer,
    asked: &'a AtomicUsize,
    stills: &'a std::sync::Mutex<Vec<String>>,
) -> Box<vision::Ask<'a>> {
    Box::new(
        move |_: &mut ClaudeCli, _: &str, _: &str, _: &Value, pngs: &[String]| {
            asked.fetch_add(1, Ordering::SeqCst);
            let still = String::from_utf8(STANDARD.decode(&pngs[0]).unwrap()).unwrap();
            stills.lock().unwrap().push(still);
            Ok::<_, LlmError>(Completion {
                json: serde_json::to_value(answer).unwrap(),
                input_tokens: 1,
                output_tokens: 1,
                cost_usd: None,
            })
        },
    )
}

#[test]
fn a_known_sign_takes_the_library_translation_and_asks_no_model() {
    let temp = Temporary::new();
    let mut document = frame_document(vec![
        sighting(
            "palace",
            (1.0, 2.0),
            "王宮",
            K1,
            [192.0, 108.0, 576.0, 216.0],
        ),
        sighting("harbor", (6.0, 7.0), "港", K2, [192.0, 756.0, 576.0, 864.0]),
    ]);
    write_images(&temp.0, &document);
    let mut known = BTreeMap::new();
    known.insert("palace".to_string(), library_sign("王宮", "Royal Palace"));
    known.insert("harbor".to_string(), library_sign("港", "Harbor"));
    let never = |_: &mut ClaudeCli,
                 _: &str,
                 _: &str,
                 _: &Value,
                 _: &[String]|
     -> Result<Completion, LlmError> { panic!("Claude must not be asked") };
    translate_with(&temp.0, &mut document, &known, &never).unwrap();
    let palace = document
        .occurrences
        .iter()
        .find(|o| o.id == "palace")
        .unwrap();
    assert_eq!(palace.english.as_deref(), Some("Royal Palace"));
    assert_eq!(palace.provenance.backend, LIBRARY_BACKEND);
    assert!(palace.provenance.reason.contains("dressrosa-11"));
    assert_eq!(palace.confidence, 0.97);
    assert!(palace.warnings.is_empty(), "{:?}", palace.warnings);
    let harbor = document
        .occurrences
        .iter()
        .find(|o| o.id == "harbor")
        .unwrap();
    assert_eq!(harbor.english.as_deref(), Some("Harbor"));
}

#[test]
fn a_keyframe_with_unknown_writing_is_asked_whole_and_the_known_sign_keeps_its_translation() {
    let temp = Temporary::new();
    let mut document = frame_document(vec![
        sighting(
            "palace",
            (1.0, 2.0),
            "王宮",
            K1,
            [192.0, 108.0, 576.0, 216.0],
        ),
        sighting(
            "pirates",
            (1.5, 2.5),
            "海賊",
            K1,
            [960.0, 540.0, 1344.0, 648.0],
        ),
        sighting("harbor", (6.0, 7.0), "港", K2, [192.0, 756.0, 576.0, 864.0]),
    ]);
    write_images(&temp.0, &document);
    let mut known = BTreeMap::new();
    known.insert("palace".to_string(), library_sign("王宮", "Royal Palace"));
    known.insert("harbor".to_string(), library_sign("港", "Harbor"));
    let answer = KeyframeAnswer {
        regions: vec![
            region("r1", "王宮", Some("The Palace"), 0.6, [0.1, 0.1, 0.3, 0.2]),
            region("r2", "海賊", Some("Pirates"), 0.9, [0.5, 0.5, 0.7, 0.6]),
        ],
        other_text: Vec::new(),
    };
    let asked = AtomicUsize::new(0);
    let stills = std::sync::Mutex::new(Vec::new());
    let ask = answering(&answer, &asked, &stills);
    translate_with(&temp.0, &mut document, &known, &*ask).unwrap();
    drop(ask);
    assert_eq!(
        asked.load(Ordering::SeqCst),
        1,
        "only the keyframe with unknown writing"
    );
    assert_eq!(
        stills.into_inner().unwrap(),
        vec![format!("png bytes of {K1}")]
    );
    let find = |id: &str| document.occurrences.iter().find(|o| o.id == id).unwrap();
    assert_eq!(find("palace").english.as_deref(), Some("Royal Palace"));
    assert_eq!(find("palace").provenance.backend, LIBRARY_BACKEND);
    assert_eq!(find("palace").confidence, 0.97);
    assert_eq!(find("pirates").english.as_deref(), Some("Pirates"));
    assert_eq!(find("pirates").provenance.backend, "claude-cli/test");
}

#[test]
fn with_no_known_sign_every_keyframe_is_asked() {
    let temp = Temporary::new();
    let document = frame_document(vec![
        sighting(
            "palace",
            (1.0, 2.0),
            "王宮",
            K1,
            [192.0, 108.0, 576.0, 216.0],
        ),
        sighting("harbor", (6.0, 7.0), "港", K2, [192.0, 756.0, 576.0, 864.0]),
    ]);
    let mut requests =
        super::super::tests::build_requests(&temp.0, &document, &TextCorrections::default(), &[]);
    skip_known(&mut requests, &BTreeMap::new());
    assert_eq!(requests.len(), 2);
    let mut known = BTreeMap::new();
    known.insert("palace".to_string(), library_sign("王宮", "Royal Palace"));
    skip_known(&mut requests, &known);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].regions[0].id, "harbor");
}
