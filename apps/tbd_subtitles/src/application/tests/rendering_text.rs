//! Headless visual-review flow through the real application, with a stand-in pipeline.

use std::fs;
use std::path::Path;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use job_model::job::{JobRecord, JobSettings, StepMeasure, StepRecord};
use job_model::onscreen::{
    Point, Quad, TextCorrections, TextDocument, TextFrame, TextOccurrence, TextProvenance,
    TextSettings, TextTreatment,
};
use job_model::outputs::{AudioStream, OutputRecord, ProbeDecoded, ProbeResult, VideoStream};
use pipeline::work_dir::{self, WorkDir};

use super::*;

struct Fixture {
    root: PathBuf,
    video: PathBuf,
    work: WorkDir,
    calls: Arc<Mutex<Vec<PathBuf>>>,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("tbd-text-ui-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("fixture folder");
        let video = root.join("Dressrosa visual pilot.mkv");
        fs::write(&video, b"stand-in video, never changed").expect("source fixture");
        let work = WorkDir::new(root.join("work").join(work_dir::job_id(&video)));
        let mut settings = JobSettings::with_glossary(Vec::new());
        settings.onscreen_text = TextSettings::new_job();
        settings.onscreen_text.localized_video = false;
        let store = store(&work);
        store
            .put_job_record(&JobRecord {
                video: video.to_string_lossy().into_owned(),
                video_size: 28,
                video_modified_s: 0,
                settings,
                models_dir: Some(root.join("models").to_string_lossy().into_owned()),
                corrections: None,
            })
            .expect("job record");
        store
            .put_step_record(
                StepName::TextDetect,
                &StepRecord {
                    fingerprint: "fixture".into(),
                    finished_ns: 1,
                    measure: StepMeasure {
                        wall_s: 90.0,
                        ..Default::default()
                    },
                },
            )
            .expect("the step");
        let track = AudioStream {
            index: 1,
            audio_position: 0,
            codec: "aac".into(),
            language: Some("eng".into()),
            channels: 2,
            sample_rate: 48_000,
            start_time_s: 0.0,
        };
        let probe = ProbeDecoded {
            probe: ProbeResult {
                duration_s: 30.0,
                video: Some(VideoStream {
                    index: 0,
                    codec: "h264".into(),
                    width: 1920,
                    height: 1080,
                    frame_rate_num: 24,
                    frame_rate_den: 1,
                    start_time_s: 0.0,
                    ..Default::default()
                }),
                audio: vec![track.clone()],
            },
            track,
            samples: 480_000,
        };
        store
            .put_output(StepName::ProbeDecode, None, &probe)
            .expect("probe");
        let qc = QcReport {
            summary: job_model::report::QcSummary {
                video_s: 30.0,
                ..Default::default()
            },
            ..Default::default()
        };
        store.put_output(StepName::Qc, None, &qc).expect("qc");
        let output = OutputRecord {
            path: video.with_extension("ass").to_string_lossy().into_owned(),
            ..Default::default()
        };
        store
            .put_output(StepName::Output, None, &output)
            .expect("output");
        let crop = work.root().join("visual/board.png");
        fs::create_dir_all(crop.parent().expect("crop parent")).expect("visual folder");
        image::RgbImage::from_pixel(120, 48, image::Rgb([230, 222, 200]))
            .save(&crop)
            .expect("thumbnail fixture");
        let mut board = occurrence(
            "board",
            "ドレスローザ SOP作戦",
            Some("Dressrosa SOP Operation"),
        );
        board.crops.push(PathBuf::from("visual/board.png"));
        board.presentation.treatment = TextTreatment::Nearby;
        board
            .warnings
            .push("Character occlusion: nearby translation needs review".into());
        let mut title = occurrence("title", "運命の再会", Some("Fated Reunion"));
        title.start_s = 14.0;
        title.end_s = 16.0;
        let mut name = occurrence("name", "レベッカ", None);
        name.start_s = 20.0;
        name.end_s = 22.0;
        name.warnings
            .push("Reading uncertain; verify this name".into());
        name.rendered = Some(false);
        let document = TextDocument {
            review_warnings: Vec::new(),
            proxy_width: 0,
            sample_step: 0,
            width: 1920,
            height: 1080,
            decoded_frames: 720,
            occurrences: vec![board, title, name],
        };
        store
            .put_output(StepName::TextTranslate, None, &document)
            .expect("translated");
        store
            .put_output(StepName::TextTypeset, None, &document)
            .expect("typeset");
        fs::write(video.with_extension("ass"), ass(&document)).expect("export fixture");
        Self {
            root,
            video,
            work,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn runner(&self) -> RunJob {
        let calls = self.calls.clone();
        Arc::new(move |video, options, progress| {
            assert!(options.settings.onscreen_text.enabled);
            assert!(
                options.rerun.is_empty(),
                "corrections must not force audio stages"
            );
            let work = WorkDir::new(options.work_root.join(work_dir::job_id(video)));
            let store = store(&work);
            let corrections = work_dir::read_text_corrections(&store).expect("saved corrections");
            let mut document = document(&store, StepName::TextTranslate);
            stages::onscreen_text::review::apply(&mut document, &corrections, 30.0)
                .expect("real review applies saved edits");
            store
                .put_output(StepName::TextTypeset, None, &document)
                .expect("typeset");
            fs::write(video.with_extension("ass"), ass(&document)).expect("stand-in output");
            progress(Progress::StepStarted(StepName::TextReview));
            calls.lock().expect("calls").push(video.to_path_buf());
            Ok(JobOutcome {
                work_dir: work.root().to_path_buf(),
                subtitles: video.with_extension("ass"),
                report: work.root().join("report.md"),
                qc: QcReport::default(),
                ran: vec![
                    StepName::TextReview,
                    StepName::TextTypeset,
                    StepName::Qc,
                    StepName::Output,
                ],
                skipped: vec![StepName::AsrWhisper],
            })
        })
    }

    fn harness(&self, gpu: bool) -> Harness<'static, TbdSubtitlesApp> {
        self.harness_with(gpu, self.runner())
    }

    fn harness_with(&self, gpu: bool, run: RunJob) -> Harness<'static, TbdSubtitlesApp> {
        let root = self.root.clone();
        let video = self.video.clone();
        let builder = Harness::builder().with_size(egui::vec2(1400.0, 1200.0));
        let builder = if gpu { builder.wgpu() } else { builder };
        let mut harness = builder.build_eframe(move |creation| {
            theme::install(&creation.egui_ctx);
            let mut app = TbdSubtitlesApp::new(Environment::scratch(&root, run), vec![video]);
            for model in &mut app.settings.items {
                model.present = true;
            }
            app.queue.items[0].state = JobState::FinishedBefore;
            app.queue.selected = Some(app.queue.items[0].id);
            app.refresh_report(true);
            app
        });
        harness.run_steps(2);
        harness
    }
}

impl Fixture {
    /// The on-screen text corrections the job's database holds.
    fn corrections(&self) -> TextCorrections {
        work_dir::read_text_corrections(&store(&self.work)).expect("saved corrections")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// This process's handle of the database of the job in `work`.
fn store(work: &WorkDir) -> Arc<work_dir::JobStore> {
    work_dir::JobStore::open(work).expect("the store")
}

/// `step`'s on-screen text document in `store`, which must be there.
fn document(store: &work_dir::JobStore, step: StepName) -> TextDocument {
    store
        .read()
        .expect("read")
        .output(step, None)
        .expect("a stored document")
        .expect("the document is there")
}

fn occurrence(id: &str, japanese: &str, english: Option<&str>) -> TextOccurrence {
    let quad = Quad([
        Point { x: 150.0, y: 100.0 },
        Point { x: 900.0, y: 100.0 },
        Point { x: 900.0, y: 260.0 },
        Point { x: 150.0, y: 260.0 },
    ]);
    TextOccurrence {
        source_fingerprint: None,
        id: id.into(),
        start_s: 10.0,
        end_s: 12.0,
        japanese: japanese.into(),
        english: english.map(str::to_owned),
        confidence: 0.82,
        crops: Vec::new(),
        frames: (0..48)
            .map(|frame| TextFrame {
                time_s: 10.0 + frame as f64 / 24.0,
                end_s: 10.0 + (frame + 1) as f64 / 24.0,
                quad,
                confidence: 0.95,
                surface_rgb: None,
            })
            .collect(),
        provenance: TextProvenance {
            backend: "qwen3.5-4b".into(),
            reference: Some(PathBuf::from("verified/reference.ass")),
            reason: "Visible wording matched to this scene".into(),
        },
        presentation: Default::default(),
        warnings: Vec::new(),
        reviewed: false,
        rendered: Some(true),
        keyframe: None,
        ruby: Vec::new(),
    }
}

fn ass(document: &TextDocument) -> String {
    let mut output = "[Script Info]\nScriptType: v4.00+\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n".to_string();
    for text in &document.occurrences {
        if let Some(english) = &text.english {
            output.push_str(&format!(
                "Dialogue: 0,0:00:10.00,0:00:12.00,Default,,0,0,0,,{english}\n"
            ));
        }
    }
    output
}

fn wait(harness: &mut Harness<'_, TbdSubtitlesApp>, ready: impl Fn(&TbdSubtitlesApp) -> bool) {
    for _ in 0..500 {
        harness.run_steps(1);
        if ready(harness.state()) {
            harness.run_steps(2);
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!(
        "visual UI did not settle; error: {:?}",
        harness.state().text.error
    );
}

fn open_text(harness: &mut Harness<'_, TbdSubtitlesApp>) {
    harness
        .get_all_by_label("Check Text")
        .max_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
        .expect("Overview review action")
        .click();
    wait(harness, |app| {
        app.text.session.is_some() && app.text.pending.is_none()
    });
}

fn saved(harness: &mut Harness<'_, TbdSubtitlesApp>, fixture: &Fixture, count: usize) {
    wait(harness, |app| {
        fixture.calls.lock().expect("calls").len() == count
            && app.text.saving.is_none()
            && app.text.pending.is_none()
            && app
                .queue
                .items
                .iter()
                .all(|item| !matches!(item.state, JobState::Running(_)))
            && app.text.session.is_some()
    });
}

#[test]
fn completed_visual_job_opens_text_review_with_counts_provenance_flags_and_thumbnail() {
    let fixture = Fixture::new("overview");
    let mut harness = fixture.harness(false);
    let (text, _) = render(harness.state());
    for expected in [
        "3 detected · 2 translated · 1 nearby · 1 unresolved",
        "Visual processing: 1.5 min · Combined ASS output",
        "2 visual items need review",
        "Dressrosa visual pilot.ass",
    ] {
        assert!(text.contains(expected), "missing {expected}: {text}");
    }
    open_text(&mut harness);
    let app = harness.state();
    assert_eq!(app.detail_tab(app.queue.items[0].id), DetailTab::CheckText);
    assert!(app.text.session.as_ref().expect("session").thumbnails[0].is_some());
    let (text, _) = render(app);
    assert!(
        !text.contains("Unsaved changes"),
        "opening a saved occurrence must not mark its unchanged draft dirty"
    );
    for expected in [
        "ドレスローザ SOP作戦",
        "Dressrosa SOP Operation",
        "qwen3.5-4b",
        "82% confidence",
        "verified/reference.ass",
        "Character occlusion",
        "Original",
        "English subtitles",
        "Save & regenerate ASS",
        "Retry selected text",
    ] {
        assert!(text.contains(expected), "missing {expected}: {text}");
    }
}

#[test]
fn english_edit_save_undo_and_retries_use_the_same_video_and_preserve_source() {
    let fixture = Fixture::new("corrections");
    let source = fs::read(&fixture.video).expect("source before");
    let mut harness = fixture.harness(false);
    open_text(&mut harness);
    harness
        .get_by_role(egui::accesskit::Role::MultilineTextInput)
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.event(egui::Event::Text("Dressrosa: Operation SOP".into()));
    harness.run_steps(2);
    assert!(render(harness.state()).0.contains("Unsaved changes"));
    assert_eq!(
        harness
            .state()
            .text
            .session
            .as_ref()
            .expect("session")
            .draft
            .as_ref()
            .expect("edited draft")
            .english
            .as_deref(),
        Some("Dressrosa: Operation SOP")
    );
    harness.get_by_label("Save & regenerate ASS").click();
    saved(&mut harness, &fixture, 1);
    assert_eq!(
        harness
            .state()
            .text
            .session
            .as_ref()
            .expect("session")
            .selected,
        0,
        "saving preserves the selected sign so its Undo applies to that correction"
    );
    let corrections: TextCorrections = fixture.corrections();
    assert_eq!(
        corrections.edits["board"].english.as_deref(),
        Some("Dressrosa: Operation SOP")
    );
    assert!(
        fs::read_to_string(fixture.video.with_extension("ass"))
            .expect("ASS")
            .contains("Dressrosa: Operation SOP")
    );
    harness.get_by_label("Undo").click();
    saved(&mut harness, &fixture, 2);
    let corrections: TextCorrections = fixture.corrections();
    assert!(corrections.edits.is_empty());
    assert!(
        fs::read_to_string(fixture.video.with_extension("ass"))
            .expect("ASS")
            .contains("Dressrosa SOP Operation")
    );
    for count in 3..=4 {
        harness.get_by_label("Retry selected text").click();
        saved(&mut harness, &fixture, count);
    }
    let corrections: TextCorrections = fixture.corrections();
    assert_eq!(corrections.retry, vec!["board", "board"]);
    assert!(
        fixture
            .calls
            .lock()
            .expect("calls")
            .iter()
            .all(|video| video == &fixture.video)
    );
    assert_eq!(fs::read(&fixture.video).expect("source after"), source);
}

#[test]
fn preview_controls_scrub_and_step_source_frames_without_blocking_navigation() {
    let fixture = Fixture::new("transport");
    let mut harness = fixture.harness(false);
    open_text(&mut harness);
    let time = |harness: &Harness<'_, TbdSubtitlesApp>| {
        harness
            .state()
            .text
            .session
            .as_ref()
            .expect("session")
            .position_s
    };
    let start = time(&harness);
    harness.get_by_label("Next frame").click();
    harness.run_steps(2);
    assert!((time(&harness) - start - 1.0 / 24.0).abs() < 1e-9);
    harness.get_by_label("Previous frame").click();
    harness.run_steps(2);
    assert!((time(&harness) - start).abs() < 1e-9);
    harness.get_by_label("Preview position").focus();
    harness.run_steps(1);
    harness.key_press(egui::Key::ArrowRight);
    harness.run_steps(2);
    assert!(
        time(&harness) > start,
        "scrubbing must update requested source time"
    );
    harness.get_by_label("Play").click();
    harness.run_steps(2);
    harness.get_by_label("Overview").click();
    harness.run_steps(2);
    assert!(harness.state().text.session.is_none());
    assert!(
        harness.state().text.player.is_none(),
        "leaving the view cancels preview work"
    );
}

/// Turn the fixture's localized video on, with `board` drawn in (its plate and mask on disk) and
/// the rest left out; `board` asks to be replaced.
fn localize(fixture: &Fixture) {
    use job_model::onscreen::{
        PixelRect, Plate, ReplaceStatus, ReplacedText, ReplacementDocument, VerifiedReplacements,
    };
    let store = store(&fixture.work);
    let mut record = work_dir::load_job_record(&store)
        .expect("read")
        .expect("job record");
    record.settings.onscreen_text.localized_video = true;
    store.put_job_record(&record).expect("job record");
    let mut document = document(&store, StepName::TextTypeset);
    document.occurrences[0].presentation.treatment = TextTreatment::Replace;
    store
        .put_output(StepName::TextTypeset, None, &document)
        .expect("typeset");
    let folder = fixture.work.root().join("visual/patches/board");
    fs::create_dir_all(&folder).expect("patch folder");
    image::RgbImage::from_pixel(750, 160, image::Rgb([240, 240, 240]))
        .save(folder.join("preview.png"))
        .expect("preview");
    image::GrayImage::from_pixel(750, 160, image::Luma([255]))
        .save(folder.join("mask.png"))
        .expect("mask");
    let text = |id: &str, status: ReplaceStatus| ReplacedText {
        id: id.into(),
        first_frame: 240,
        last_frame: 287,
        status,
        style: None,
        container: None,
        plates: Vec::new(),
        preview: None,
        lettering_quad: None,
    };
    let mut board = text("board", ReplaceStatus::Baked);
    board.preview = Some(PathBuf::from("visual/patches/board/preview.png"));
    board.plates.push(Plate {
        first_frame: 240,
        last_frame: 287,
        rect: PixelRect {
            x: 150,
            y: 100,
            width: 750,
            height: 160,
        },
        shift: [0.0, 0.0],
        scale: 1.0,
        source: PathBuf::from("visual/patches/board/preview.png"),
        mask: PathBuf::from("visual/patches/board/mask.png"),
        plate: None,
        patch: None,
    });
    let verified = VerifiedReplacements {
        document: ReplacementDocument {
            width: 1920,
            height: 1080,
            frame_count: 720,
            texts: vec![
                board,
                text(
                    "title",
                    ReplaceStatus::Fallback("the card is too busy".into()),
                ),
            ],
        },
        checks: Vec::new(),
    };
    store
        .put_output(StepName::TextVerify, None, &verified)
        .expect("verified");
}

#[test]
fn a_localized_job_previews_the_localized_video_its_mask_and_whether_text_was_replaced() {
    let fixture = Fixture::new("localized");
    localize(&fixture);
    let mut harness = fixture.harness(false);
    open_text(&mut harness);
    wait(&mut harness, |app| {
        let session = app.text.session.as_ref();
        session
            .and_then(|session| session.localized.as_ref())
            .is_some_and(|localized| localized.pictures.is_some())
    });
    let (text, _) = render(harness.state());
    for expected in [
        "Original",
        "Show erase mask",
        "Subtitles",
        "Localized video",
        "Localized video not written yet",
        "Replaced in the video",
        "Replace in the video",
    ] {
        assert!(text.contains(expected), "missing {expected}: {text}");
    }
    assert!(!text.contains("English subtitles"), "{text}");
    harness.get_by_label("Show erase mask").click();
    harness.run_steps(2);
    let localized = |harness: &Harness<'_, TbdSubtitlesApp>| {
        let session = harness.state().text.session.as_ref().expect("session");
        session.localized.clone().expect("localized")
    };
    assert!(localized(&harness).show_mask);
    harness.get_by_label("Subtitles").click();
    harness.run_steps(2);
    assert_eq!(
        localized(&harness).mode,
        crate::text_review::models::PreviewMode::Subtitles
    );
    let (text, _) = render(harness.state());
    assert!(!text.contains("Localized video not written yet"), "{text}");
    // The title is not flagged, so the list hides it; select it as its row would.
    harness
        .state_mut()
        .apply_text(crate::text_review::models::Event::Select(1));
    harness.run_steps(2);
    let (text, _) = render(harness.state());
    assert!(
        text.contains("Not replaced in the video: the card is too busy"),
        "{text}"
    );
}

#[test]
fn a_job_without_a_localized_video_keeps_the_subtitles_preview_alone() {
    let fixture = Fixture::new("not-localized");
    let mut harness = fixture.harness(false);
    open_text(&mut harness);
    let (text, _) = render(harness.state());
    assert!(text.contains("English subtitles"), "{text}");
    for absent in [
        "Show erase mask",
        "Localized video",
        "in the video",
        "Replace in the video",
    ] {
        assert!(!text.contains(absent), "{absent} in {text}");
    }
}

#[test]
#[ignore = "renders visual-review screenshots through a host GPU; set TBD_SNAPSHOTS"]
fn visual_review_snapshots() {
    let fixture = Fixture::new("snapshots");
    let out = PathBuf::from(std::env::var_os("TBD_SNAPSHOTS").expect("TBD_SNAPSHOTS folder"));
    fs::create_dir_all(&out).expect("screenshot folder");
    let mut harness = fixture.harness(true);
    for (scheme, name) in [(Scheme::Light, "light"), (Scheme::Dark, "dark")] {
        harness.state_mut().scheme = scheme;
        harness.run_steps(3);
        harness
            .render()
            .expect("render Overview")
            .save(out.join(format!("m4-overview-{name}.png")))
            .expect("save Overview");
    }
    open_text(&mut harness);
    for (scheme, name) in [(Scheme::Light, "light"), (Scheme::Dark, "dark")] {
        harness.state_mut().scheme = scheme;
        harness.run_steps(3);
        harness
            .render()
            .expect("render Check Text")
            .save(out.join(format!("m4-check-text-{name}.png")))
            .expect("save Check Text");
    }
}

#[path = "rendering_text_pilot.rs"]
mod rendering_text_pilot;
