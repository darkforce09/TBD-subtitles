use std::path::PathBuf;

use job_model::job::{JobRecord, JobSettings};
use job_model::onscreen::{
    PixelRect, Plate, ReplaceStatus, ReplacedText, TextCheck, VerifiedReplacements,
};

use super::*;
use crate::library::Library;
use crate::library::fixtures::{hash, occurrence, style};
use crate::work_dir::store::scratch::Scratch;

/// `id`'s replacement with `status`, one plate whose patch and mask are files of the job.
fn replaced(scratch: &Scratch, id: &str, status: ReplaceStatus) -> ReplacedText {
    let patch = format!("visual/patches/{id}/patch.png");
    let mask = format!("visual/masks/{id}/mask.png");
    scratch.file(&patch, b"patch");
    scratch.file(&mask, b"mask");
    ReplacedText {
        id: id.into(),
        first_frame: 0,
        last_frame: 10,
        status,
        style: Some(style()),
        container: None,
        plates: vec![Plate {
            first_frame: 0,
            last_frame: 10,
            rect: PixelRect {
                x: 0,
                y: 0,
                width: 64,
                height: 32,
            },
            shift: [0.0, 0.0],
            scale: 1.0,
            source: PathBuf::from(format!("visual/masks/{id}/source.png")),
            mask: PathBuf::from(mask),
            plate: None,
            patch: Some(PathBuf::from(patch)),
            shifted: Vec::new(),
        }],
        preview: None,
        lettering_quad: None,
    }
}

fn check(id: &str, passed: bool) -> TextCheck {
    TextCheck {
        id: id.into(),
        samples: 1,
        passed,
    }
}

#[test]
fn the_output_records_the_replacements_that_read_back_in_the_library() {
    let scratch = Scratch::new("layout-library");
    let (store, dir) = (scratch.store(), scratch.dir.clone());
    let library = Library::at(dir.join("data").join(crate::library::FILE_NAME));
    let mut settings = JobSettings::with_glossary(vec![]);
    settings.onscreen_text.enabled = true;
    settings.onscreen_text.localized_video = true;
    let video = dir.join("episode.mkv");
    let job = Job {
        work: scratch.work().clone(),
        record: JobRecord {
            video: video.to_string_lossy().into_owned(),
            video_size: 1,
            video_modified_s: 0,
            settings,
            models_dir: Some("/no/models".into()),
            corrections: None,
        },
        library: Some(library.clone()),
    };
    let track = CueTrack {
        frame_rate: FrameRate::FILM,
        cues: Vec::new(),
    };
    store.put_output(StepName::Cues, None, &track).unwrap();
    store
        .put_output(
            StepName::TextTypeset,
            Some(keys::TYPESET_ASS),
            &String::new(),
        )
        .unwrap();
    let text = TextDocument {
        width: 1920,
        height: 1080,
        occurrences: vec![
            occurrence(&dir, "passed", "王宮", 1),
            occurrence(&dir, "failed", "港", 2),
        ],
        ..TextDocument::default()
    };
    store
        .put_output(StepName::TextTypeset, None, &text)
        .unwrap();
    let verified = VerifiedReplacements {
        document: ReplacementDocument {
            width: 1920,
            height: 1080,
            frame_count: 100,
            texts: vec![
                replaced(&scratch, "passed", ReplaceStatus::Baked),
                replaced(&scratch, "failed", ReplaceStatus::Baked),
            ],
        },
        checks: vec![check("passed", true), check("failed", false)],
    };
    store
        .put_output(StepName::TextVerify, None, &verified)
        .unwrap();

    let mut io = crate::tasks::StepIo::in_process(store).unwrap();
    let report = output(&job, &mut io, &|_, _| {}).expect("output");
    assert_eq!(
        report.notes.get("library_added").map(String::as_str),
        Some("1")
    );
    let sign = library.lookup("王宮", hash(1)).unwrap().expect("recorded");
    assert_eq!(sign.english, "English of passed");
    assert_eq!(sign.episodes, vec![job.id()]);
    assert_eq!(sign.patch_png, b"patch");
    assert_eq!(library.lookup("港", hash(2)).unwrap(), None);
    assert_eq!(library.size().unwrap().signs, 1);
}
