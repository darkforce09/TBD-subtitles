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
        write(
            &work.job_json(),
            &JobRecord {
                video: video.to_string_lossy().into_owned(),
                video_size: 28,
                video_modified_s: 0,
                settings,
                models_dir: Some(root.join("models").to_string_lossy().into_owned()),
                corrections: None,
                steps: [(
                    StepName::TextDetect,
                    StepRecord {
                        fingerprint: "fixture".into(),
                        finished_ns: 1,
                        measure: StepMeasure {
                            wall_s: 90.0,
                            ..Default::default()
                        },
                    },
                )]
                .into(),
            },
        );
        let track = AudioStream {
            index: 1,
            audio_position: 0,
            codec: "aac".into(),
            language: Some("eng".into()),
            channels: 2,
            sample_rate: 48_000,
            start_time_s: 0.0,
        };
        write(
            &work.probe(),
            &ProbeDecoded {
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
                    }),
                    audio: vec![track.clone()],
                },
                track,
                samples: 480_000,
            },
        );
        write(
            &work.qc(),
            &QcReport {
                summary: job_model::report::QcSummary {
                    video_s: 30.0,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        write(
            &work.output_record(),
            &OutputRecord {
                path: video.with_extension("ass").to_string_lossy().into_owned(),
                ..Default::default()
            },
        );
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
        write(&work.text(StepName::TextTranslate), &document);
        write(&work.text(StepName::TextTypeset), &document);
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
            let corrections = fs::read(work.text_corrections())
                .map(|bytes| {
                    serde_json::from_slice::<TextCorrections>(&bytes).expect("saved corrections")
                })
                .unwrap_or_default();
            let mut document: TextDocument = read(&work.text(StepName::TextTranslate));
            stages::onscreen_text::review::apply(&mut document, &corrections, 30.0)
                .expect("real review applies saved edits");
            write(&work.text(StepName::TextTypeset), &document);
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

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write(path: &Path, value: &impl serde::Serialize) {
    work_dir::write_json(path, value).expect("fixture JSON");
}

fn read<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    serde_json::from_slice(&fs::read(path).expect("read fixture")).expect("parse fixture")
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
    let corrections: TextCorrections = read(&fixture.work.text_corrections());
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
    let corrections: TextCorrections = read(&fixture.work.text_corrections());
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
    let corrections: TextCorrections = read(&fixture.work.text_corrections());
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

/// Exercise real preview decoding only; this does not validate OCR, translation or VLC playback.
#[test]
#[ignore = "host FFmpeg/GPU/audio; set TBD_M4_PREVIEW_WORK and TBD_M4_PREVIEW_VIDEO"]
fn real_visual_preview_host() {
    let source_work = WorkDir::new(
        PathBuf::from(std::env::var_os("TBD_M4_PREVIEW_WORK").expect("finished pilot work folder"))
            .canonicalize()
            .expect("existing pilot work folder"),
    );
    let source_video =
        PathBuf::from(std::env::var_os("TBD_M4_PREVIEW_VIDEO").expect("pilot video clip"))
            .canonicalize()
            .expect("existing pilot video clip");
    let video_before = fs::metadata(&source_video).expect("source video metadata");
    let (source_ass_path, source_ass) =
        pilot_ass(&source_work).expect("actual exported or pilot ASS");
    let source_document = fs::read(source_work.text(StepName::TextTypeset)).expect("pilot typeset");
    let mut fixture = Fixture::new("real-preview");
    install_preview_pilot(&mut fixture, &source_work, &source_video, &source_ass);
    let snapshots = std::env::var_os("TBD_SNAPSHOTS").map(PathBuf::from);
    let mut harness = fixture.harness_with(
        snapshots.is_some(),
        Arc::new(|_, _, _| panic!("a read-only preview test must never run the pipeline")),
    );
    open_text(&mut harness);
    let (selected, fps, initial_time) = {
        let session = harness.state().text.session.as_ref().expect("loaded pilot");
        assert_eq!(session.video, source_video);
        assert_eq!(session.ass, fixture.root.join("pilot.ass"));
        let requested = std::env::var("TBD_M4_PREVIEW_TEXT").ok();
        let (index, occurrence) = session
            .document
            .occurrences
            .iter()
            .enumerate()
            .filter(|(_, text)| {
                text.rendered == Some(true)
                    && text
                        .english
                        .as_ref()
                        .is_some_and(|text| !text.trim().is_empty())
                    && text.end_s - text.start_s >= 0.8
                    && requested.as_ref().is_none_or(|id| id == &text.id)
            })
            .max_by(|(_, a), (_, b)| (a.end_s - a.start_s).total_cmp(&(b.end_s - b.start_s)))
            .expect("pilot needs a rendered English occurrence lasting at least 0.8 seconds");
        assert!(session.thumbnails[index].is_some(), "pilot thumbnail loads");
        let time = occurrence.start_s + (2.0 / session.fps).max(0.1);
        eprintln!(
            "Real preview: {} at {time:.3}s, {:.3} fps, exported ASS {}",
            occurrence.id,
            session.fps,
            source_ass_path.display()
        );
        (index, session.fps, time)
    };
    harness
        .state_mut()
        .apply_text(crate::text_review::models::Event::Select(selected));
    harness
        .state_mut()
        .apply_text(crate::text_review::models::Event::Seek(initial_time));
    let first = wait_real_comparison(&mut harness, initial_time);
    assert_preview_pixels(&first, true);

    harness.get_by_label("Next frame").click();
    harness.run_steps(2);
    let next_time = preview_position(&harness);
    assert!(next_time > initial_time);
    assert!(next_time - initial_time <= 2.0 / fps + 0.001);
    let next = wait_real_comparison(&mut harness, next_time);
    assert_ne!(next.original.serial, first.original.serial);
    assert_preview_pixels(&next, false);
    harness.get_by_label("Previous frame").click();
    harness.run_steps(2);
    let previous_time = preview_position(&harness);
    assert!(previous_time < next_time);
    let _ = wait_real_comparison(&mut harness, previous_time);

    harness.get_by_label("Preview position").focus();
    harness.run_steps(1);
    harness.key_press(egui::Key::ArrowRight);
    harness.run_steps(2);
    let scrubbed_time = preview_position(&harness);
    assert!(
        scrubbed_time > previous_time,
        "scrubbing moves the requested time"
    );
    let _ = wait_real_comparison(&mut harness, scrubbed_time);
    harness.get_by_label("Play").click();
    wait_real_preview(&mut harness, |app| {
        app.text.player.as_ref().is_some_and(|player| {
            player.is_playing()
                && player
                    .comparison()
                    .is_some_and(|pair| pair.time_s >= scrubbed_time + 0.25)
        })
    });
    let playing = preview_comparison(harness.state()).expect("playing comparison");
    assert_preview_pixels(&playing, false);
    harness.get_by_label("Stop").click();
    wait_real_preview(&mut harness, |app| {
        app.text
            .player
            .as_ref()
            .is_some_and(|player| !player.is_playing())
    });

    // Capture a still with visible exported text instead of a possibly transitional playback frame.
    harness
        .state_mut()
        .apply_text(crate::text_review::models::Event::Seek(initial_time));
    let displayed = wait_real_comparison(&mut harness, initial_time);
    assert_preview_pixels(&displayed, true);
    if let Some(folder) = snapshots {
        fs::create_dir_all(&folder).expect("real preview screenshot folder");
        for (scheme, name) in [(Scheme::Light, "light"), (Scheme::Dark, "dark")] {
            harness.state_mut().scheme = scheme;
            harness.run_steps(3);
            harness
                .render()
                .expect("render real Check Text preview")
                .save(folder.join(format!("m4-real-check-text-{name}.png")))
                .expect("save real Check Text screenshot");
        }
    }
    harness.get_by_label("Overview").click();
    harness.run_steps(2);
    assert!(harness.state().text.player.is_none());
    assert!(harness.state().text.session.is_none());
    assert!(fixture.calls.lock().expect("calls").is_empty());
    let video_after = fs::metadata(&source_video).expect("source video remains");
    assert_eq!(video_after.len(), video_before.len());
    assert_eq!(
        video_after.modified().unwrap(),
        video_before.modified().unwrap()
    );
    assert_eq!(fs::read(&source_ass_path).unwrap(), source_ass);
    assert_eq!(
        fs::read(source_work.text(StepName::TextTypeset)).unwrap(),
        source_document
    );
}

fn pilot_ass(source: &WorkDir) -> Result<(PathBuf, Vec<u8>), String> {
    let installed = fs::read(source.output_record())
        .ok()
        .and_then(|bytes| serde_json::from_slice::<OutputRecord>(&bytes).ok())
        .map(|record| PathBuf::from(record.path));
    installed
        .into_iter()
        .chain([source.root().join("preview.ass")])
        .find_map(|path| {
            let bytes = fs::read(&path).ok()?;
            let text = std::str::from_utf8(&bytes).ok()?;
            (text.contains("[Script Info]") && text.contains("[Events]")).then_some((path, bytes))
        })
        .ok_or_else(|| {
            format!(
                "No valid exported ASS or visual_validation preview.ass in {}",
                source.root().display()
            )
        })
}

#[test]
fn pilot_fixture_accepts_the_validation_layout_and_refuses_missing_or_invalid_ass() {
    let fixture = Fixture::new("pilot-layout");
    let installed = fixture.video.with_extension("ass");
    let expected = fs::read(&installed).unwrap();
    assert_eq!(pilot_ass(&fixture.work).unwrap().0, installed);
    fs::remove_file(fixture.work.output_record()).unwrap();
    assert!(pilot_ass(&fixture.work).is_err());
    let preview = fixture.work.root().join("preview.ass");
    fs::write(&preview, b"incomplete output").unwrap();
    assert!(pilot_ass(&fixture.work).is_err());
    fs::write(&preview, &expected).unwrap();
    assert_eq!(
        pilot_ass(&fixture.work).unwrap(),
        (preview.clone(), expected)
    );
    assert!(
        !fixture.work.output_record().exists(),
        "the pilot is read-only"
    );
    fs::remove_file(preview).unwrap();
    assert!(pilot_ass(&fixture.work).is_err());
}

fn install_preview_pilot(fixture: &mut Fixture, source: &WorkDir, video: &Path, ass: &[u8]) {
    let mut job: JobRecord = read(&source.job_json());
    assert_eq!(
        Path::new(&job.video)
            .canonicalize()
            .expect("recorded pilot video"),
        video,
        "work folder and supplied video must describe the same source timeline"
    );
    let probe: ProbeDecoded = read(&source.probe());
    let mut document: TextDocument = read(&source.text(StepName::TextTypeset));
    assert!(
        document.occurrences.len() <= 1000,
        "use a bounded pilot clip"
    );
    fixture.video = video.into();
    fixture.work = WorkDir::new(fixture.root.join("work").join(work_dir::job_id(video)));
    job.video = video.to_string_lossy().into_owned();
    write(&fixture.work.job_json(), &job);
    write(&fixture.work.probe(), &probe);
    let mut copied_bytes = 0;
    for (index, text) in document.occurrences.iter_mut().enumerate() {
        let crop = text.crops.iter().find_map(|relative| {
            if relative.components().any(|part| {
                !matches!(
                    part,
                    std::path::Component::Normal(_) | std::path::Component::CurDir
                )
            }) {
                return None;
            }
            let crop = source.root().join(relative).canonicalize().ok()?;
            crop.starts_with(source.root()).then_some(crop)
        });
        text.crops.clear();
        if let Some(crop) = crop {
            let bytes = fs::metadata(&crop).expect("pilot crop metadata").len();
            copied_bytes += bytes;
            assert!(
                copied_bytes <= 64 * 1024 * 1024,
                "pilot crop copy exceeds 64 MiB"
            );
            let relative = PathBuf::from(format!("visual/pilot-{index}.png"));
            let destination = fixture.work.root().join(&relative);
            fs::create_dir_all(destination.parent().unwrap()).expect("scratch crop folder");
            fs::copy(crop, destination).expect("copy read-only pilot crop");
            text.crops.push(relative);
        }
    }
    write(&fixture.work.text(StepName::TextTypeset), &document);
    let mut qc: QcReport = if source.qc().is_file() {
        read(&source.qc())
    } else {
        QcReport::default()
    };
    qc.summary.video_s = probe.probe.duration_s;
    write(&fixture.work.qc(), &qc);
    let ass_path = fixture.root.join("pilot.ass");
    fs::write(&ass_path, ass).expect("scratch copy of exported ASS");
    write(
        &fixture.work.output_record(),
        &OutputRecord {
            path: ass_path.to_string_lossy().into_owned(),
            ..Default::default()
        },
    );
}

fn preview_position(harness: &Harness<'_, TbdSubtitlesApp>) -> f64 {
    harness
        .state()
        .text
        .session
        .as_ref()
        .expect("pilot session")
        .position_s
}

fn preview_comparison(app: &TbdSubtitlesApp) -> Option<crate::text_review::models::Comparison> {
    app.text
        .player
        .as_ref()
        .and_then(|player| player.comparison())
}

fn wait_real_preview(
    harness: &mut Harness<'_, TbdSubtitlesApp>,
    ready: impl Fn(&TbdSubtitlesApp) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        harness.run_steps(1);
        let app = harness.state();
        assert!(
            app.text.error.is_none(),
            "preview load error: {:?}",
            app.text.error
        );
        if let Some(player) = &app.text.player {
            assert!(
                player.error().is_none(),
                "FFmpeg preview failed: {:?}",
                player.error()
            );
        }
        if ready(app) {
            harness.run_steps(2);
            return;
        }
        assert!(Instant::now() < deadline, "real FFmpeg preview timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_real_comparison(
    harness: &mut Harness<'_, TbdSubtitlesApp>,
    time: f64,
) -> crate::text_review::models::Comparison {
    wait_real_preview(harness, |app| {
        preview_comparison(app).is_some_and(|pair| (pair.time_s - time).abs() < 0.001)
    });
    preview_comparison(harness.state()).expect("ready comparison")
}

fn assert_preview_pixels(pair: &crate::text_review::models::Comparison, translated: bool) {
    for picture in [&pair.original, &pair.rendered] {
        assert!(picture.width > 0 && picture.height > 0);
        assert!(picture.width <= 720 && picture.height <= 720);
        assert_eq!(
            picture.rgb.len(),
            (picture.width * picture.height * 3) as usize
        );
        assert!(
            picture.rgb.iter().filter(|&&value| value > 16).count() > picture.rgb.len() / 100,
            "preview must contain an actual visible scene, not a black/empty frame"
        );
    }
    assert_eq!(
        (pair.original.width, pair.original.height),
        (pair.rendered.width, pair.rendered.height)
    );
    if translated {
        let changed = pair
            .original
            .rgb
            .iter()
            .zip(&pair.rendered.rgb)
            .filter(|(left, right)| left.abs_diff(**right) > 8)
            .count();
        assert!(
            changed > 100,
            "the exported ASS must visibly change the rendered comparison"
        );
    }
}
