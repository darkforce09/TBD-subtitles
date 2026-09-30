//! Save ownership across text-session replacement, refresh and navigation.

use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use job_model::onscreen::{TextCorrections, TextDocument, TextEdit, TextOccurrence};

use crate::application::{DetailTab, Environment};
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::queue::JobKind;
use crate::job_queue::models::queue::JobState;
use crate::job_queue::services::job_runner::{RunJob, RunnerEvent};

use super::*;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    app: TbdSubtitlesApp,
    root: PathBuf,
    ids: [JobId; 2],
    videos: [PathBuf; 2],
}

impl Fixture {
    fn new() -> Self {
        Self::with_runner(Arc::new(|_, _, _| panic!("no pipeline run in save tests")))
    }

    fn with_runner(run: RunJob) -> Self {
        let root = std::env::temp_dir().join(format!(
            "tbd-text-actions-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("fixture directory");
        let videos = [root.join("original.mkv"), root.join("selected.mkv")];
        for video in &videos {
            fs::write(video, b"video").expect("video");
        }
        let environment = Environment::scratch(&root, run);
        let mut app = TbdSubtitlesApp::new(environment, videos.to_vec());
        assert!(
            app.models_missing(),
            "correction runs stay queued in these tests"
        );
        assert_eq!(app.queue.items.len(), 2);
        for item in &mut app.queue.items {
            item.state = JobState::FinishedBefore;
        }
        let ids = [app.queue.items[0].id, app.queue.items[1].id];
        Self {
            app,
            root,
            ids,
            videos,
        }
    }

    fn show(&mut self, index: usize) {
        self.app.queue.selected = Some(self.ids[index]);
        self.app.open_text(self.ids[index]);
        // Session loading is separate from correction completion; provide its completed view.
        self.app.text.pending = None;
        self.app.text.session = Some(self.session(index));
    }

    fn session(&self, index: usize) -> Session {
        let occurrence = TextOccurrence {
            source_fingerprint: None,
            id: "sign".into(),
            start_s: 1.0,
            end_s: 2.0,
            japanese: "ドレスローザ".into(),
            english: Some(format!("English {index}")),
            confidence: 0.9,
            crops: vec![],
            frames: vec![],
            provenance: Default::default(),
            presentation: Default::default(),
            warnings: vec![],
            reviewed: false,
            rendered: Some(true),
            keyframe: None,
            ruby: Vec::new(),
        };
        Session {
            work: self.root.join(format!("work-{index}")),
            video: self.videos[index].clone(),
            ass: self.videos[index].with_extension("ass"),
            draft: Some(TextEdit::from_occurrence(&occurrence)),
            document: TextDocument {
                review_warnings: Vec::new(),
                proxy_width: 0,
                sample_step: 0,
                width: 1920,
                height: 1080,
                decoded_frames: 240,
                occurrences: vec![occurrence],
            },
            corrections: TextCorrections::default(),
            duration_s: 10.0,
            fps: 24.0,
            audio_position: 0,
            selected: 0,
            position_s: 1.0,
            flagged_only: false,
            error: None,
            thumbnails: vec![],
            localized: None,
        }
    }

    fn pending_save(&mut self, index: usize) -> mpsc::Sender<Result<(), String>> {
        let (send, result) = mpsc::channel();
        self.app.text.saving = Some(PendingSave {
            video: self.videos[index].clone(),
            result,
        });
        send
    }

    fn corrections(&self, index: usize) -> usize {
        self.app
            .queue
            .items
            .iter()
            .filter(|item| item.kind == JobKind::Review && item.video == self.videos[index])
            .map(|item| item.corrections)
            .sum()
    }

    fn assert_selected_session_unchanged(&self, index: usize) {
        let actual = self.app.text.session.as_ref().expect("selected session");
        let expected = self.session(index);
        assert_eq!(actual.video, expected.video);
        assert_eq!(actual.document, expected.document);
        assert_eq!(actual.draft, expected.draft);
        assert_eq!(actual.error, None);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn draft_survives_repeated_refresh_of_the_same_video() {
    let mut fixture = Fixture::new();
    fixture.show(0);
    fixture
        .app
        .text
        .session
        .as_mut()
        .unwrap()
        .draft
        .as_mut()
        .unwrap()
        .english = Some("Owner wording".into());
    fixture.app.refresh_text();
    fixture.app.refresh_text();
    let (send, receive) = mpsc::channel();
    fixture.app.text.pending = Some(receive);
    send.send(Ok(fixture.session(0))).unwrap();
    fixture.app.poll_text();
    assert_eq!(
        fixture
            .app
            .text
            .session
            .as_ref()
            .unwrap()
            .draft
            .as_ref()
            .unwrap()
            .english
            .as_deref(),
        Some("Owner wording")
    );
}

#[test]
fn changed_source_does_not_inherit_unsaved_wording_after_refresh() {
    let mut fixture = Fixture::new();
    fixture.show(0);
    fixture
        .app
        .text
        .session
        .as_mut()
        .unwrap()
        .draft
        .as_mut()
        .unwrap()
        .english = Some("Old sign wording".into());
    fixture.app.refresh_text();
    let mut refreshed = fixture.session(0);
    refreshed.document.occurrences[0].japanese = "別の看板".into();
    refreshed.document.occurrences[0].english = Some("Different sign".into());
    refreshed.draft = Some(TextEdit::from_occurrence(
        &refreshed.document.occurrences[0],
    ));
    let (send, receive) = mpsc::channel();
    fixture.app.text.pending = Some(receive);
    send.send(Ok(refreshed)).unwrap();
    fixture.app.poll_text();
    let session = fixture.app.text.session.as_ref().unwrap();
    assert_eq!(
        session.draft.as_ref().unwrap().english.as_deref(),
        Some("Different sign")
    );
    assert!(
        session
            .error
            .as_deref()
            .unwrap()
            .contains("was not restored")
    );
}

#[test]
fn a_mismatched_saved_correction_does_not_make_an_unchanged_draft_survive_refresh() {
    let mut fixture = Fixture::new();
    fixture.show(0);
    let session = fixture.app.text.session.as_mut().expect("loaded session");
    let text = &session.document.occurrences[0];
    let mut wrong_source = text.clone();
    wrong_source.japanese = "別の看板".into();
    let mut stale_edit = TextEdit::from_occurrence(&wrong_source);
    stale_edit.english = Some("This belongs to another sign".into());
    session
        .corrections
        .edits
        .insert(text.id.clone(), stale_edit);
    fixture.app.refresh_text();
    let mut refreshed = fixture.session(0);
    refreshed.document.occurrences[0].english = Some("Fresh generated translation".into());
    refreshed.draft = Some(TextEdit::from_occurrence(
        &refreshed.document.occurrences[0],
    ));
    let (send, receive) = mpsc::channel();
    fixture.app.text.pending = Some(receive);
    send.send(Ok(refreshed)).expect("refreshed visual result");
    fixture.app.poll_text();
    assert_eq!(
        fixture
            .app
            .text
            .session
            .as_ref()
            .unwrap()
            .draft
            .as_ref()
            .unwrap()
            .english
            .as_deref(),
        Some("Fresh generated translation"),
        "a rejected saved correction must not preserve a falsely dirty old generated draft"
    );
}

#[test]
fn stopping_before_any_preview_frame_preserves_the_requested_cursor_and_draft() {
    let mut fixture = Fixture::new();
    fixture.show(0);
    let session = fixture.app.text.session.as_mut().expect("session");
    session.position_s = 1.75;
    let draft = session.draft.clone();
    assert!(fixture.app.text.player.is_none());
    fixture.app.apply_text(Event::Stop);
    let session = fixture.app.text.session.as_ref().expect("session");
    assert_eq!(session.position_s, 1.75);
    assert_eq!(session.draft, draft);
}

#[test]
#[ignore = "real host FFmpeg/audio; set TBD_M4_PREVIEW_WORK and TBD_M4_PREVIEW_VIDEO"]
fn real_visual_stop_cursor_host() {
    use egui_kittest::kittest::Queryable as _;

    let work = PathBuf::from(std::env::var_os("TBD_M4_PREVIEW_WORK").expect("pilot work folder"));
    let video = PathBuf::from(std::env::var_os("TBD_M4_PREVIEW_VIDEO").expect("pilot video"));
    let mut fixture = Fixture::new();
    let preview_work = copy_preview_pilot(&work, &fixture.root);
    let mut session = session::load(&preview_work).expect("finished pilot review session");
    assert_eq!(
        session.video.canonicalize().unwrap(),
        video.canonicalize().unwrap()
    );
    let requested_id = std::env::var("TBD_M4_PREVIEW_TEXT").ok();
    let selected = session
        .document
        .occurrences
        .iter()
        .position(|text| {
            text.rendered == Some(true)
                && text.english.is_some()
                && text.end_s - text.start_s >= 0.8
                && requested_id.as_ref().is_none_or(|id| id == &text.id)
        })
        .expect("pilot needs a rendered English occurrence lasting at least 0.8 seconds");
    select(&mut session, selected);
    session.position_s += 0.1;
    let requested = session.position_s;
    let draft = session.draft.clone();
    fixture.show(0);
    fixture.app.text.session = Some(session);
    fixture.app.apply_text(Event::Play);
    let deadline = Instant::now() + Duration::from_secs(20);
    let shown = loop {
        let player = fixture.app.text.player.as_ref().expect("playing preview");
        assert!(
            player.error().is_none(),
            "FFmpeg failed: {:?}",
            player.error()
        );
        if let Some(pair) = player.comparison()
            && pair.time_s >= requested + 0.25
        {
            break pair.time_s;
        }
        assert!(Instant::now() < deadline, "preview did not advance");
        std::thread::sleep(Duration::from_millis(5));
    };
    fixture.app.apply_text(Event::Stop);
    let player = fixture.app.text.player.as_ref().expect("stopped preview");
    assert!(!player.is_playing());
    let session = fixture.app.text.session.as_ref().expect("stopped session");
    let stopped = session.position_s;
    assert!(
        stopped >= shown,
        "Stop adopts the last shown frame instead of {requested}"
    );
    assert!(stopped - shown <= 1.0 / session.fps + 0.001);
    assert_eq!(session.draft, draft, "stopping changes no correction");

    let mut installed = false;
    let mut harness = egui_kittest::Harness::builder()
        .with_size(eframe::egui::vec2(1400.0, 1200.0))
        .build_ui_state(
            move |ui, session: &mut Session| {
                if !installed {
                    crate::core::ui::theme::install(ui.ctx());
                    installed = true;
                    return;
                }
                let mut events = Vec::new();
                crate::text_review::ui::review::show(ui, session, None, false, false, &mut events);
                for event in events {
                    if let Event::Seek(time) = event {
                        session.position_s = time;
                    }
                }
            },
            session.clone(),
        );
    harness.run_steps(3);
    harness.get_by_label("Next frame").click();
    harness.run_steps(2);
    let next = harness.state().position_s;
    assert!(
        next > stopped,
        "frame stepping continues after the stopped frame"
    );
    harness.get_by_label("Preview position").focus();
    harness.run_steps(1);
    harness.key_press(eframe::egui::Key::ArrowRight);
    harness.run_steps(2);
    assert!(
        harness.state().position_s > next,
        "slider continues from the stopped cursor"
    );
    assert_eq!(harness.state().draft, draft);
    fixture.app.close_text();
}

fn copy_preview_pilot(source: &Path, scratch: &Path) -> PathBuf {
    use job_model::StepName;
    use job_model::outputs::OutputRecord;
    use pipeline::work_dir::{self, WorkDir};

    let source = WorkDir::new(source);
    let destination = WorkDir::new(scratch.join("preview-pilot"));
    fs::create_dir_all(destination.root().join("visual")).expect("scratch preview folder");
    for (from, to) in [
        (source.job_json(), destination.job_json()),
        (source.probe(), destination.probe()),
        (
            source.text(StepName::TextTypeset),
            destination.text(StepName::TextTypeset),
        ),
    ] {
        fs::copy(&from, to).unwrap_or_else(|error| panic!("copy {}: {error}", from.display()));
    }
    let installed = fs::read(source.output_record())
        .ok()
        .and_then(|bytes| serde_json::from_slice::<OutputRecord>(&bytes).ok())
        .map(|record| PathBuf::from(record.path));
    let (path, bytes) = installed
        .into_iter()
        .chain([source.root().join("preview.ass")])
        .find_map(|path| {
            let bytes = fs::read(&path).ok()?;
            let text = std::str::from_utf8(&bytes).ok()?;
            (text.contains("[Script Info]") && text.contains("[Events]")).then_some((path, bytes))
        })
        .expect("pilot needs a valid exported ASS or visual_validation preview.ass");
    let ass = destination.root().join("preview.ass");
    fs::write(&ass, &bytes).expect("scratch copy of the actual pilot ASS");
    assert_eq!(fs::read(path).unwrap(), fs::read(&ass).unwrap());
    work_dir::write_json(
        &destination.output_record(),
        &OutputRecord {
            path: ass.to_string_lossy().into_owned(),
            ..Default::default()
        },
    )
    .expect("scratch output record");
    destination.root().into()
}

#[test]
fn switching_selection_while_saving_queues_the_original_once_and_keeps_the_new_session() {
    let mut fixture = Fixture::new();
    fixture.show(0);
    let save = fixture.pending_save(0);
    fixture.show(1);
    assert!(fixture.app.text.saving.is_none());
    assert_eq!(fixture.app.text.parked_saves.len(), 1);
    fixture.app.poll_text();
    assert_eq!(fixture.corrections(0), 0);
    save.send(Ok(()))
        .expect("save finishes after selection changes");
    fixture.app.poll_text();
    fixture.app.poll_text();
    assert_eq!((fixture.corrections(0), fixture.corrections(1)), (1, 0));
    fixture.assert_selected_session_unchanged(1);
    assert!(fixture.app.text.parked_saves.is_empty());
}

#[test]
fn refresh_preserves_the_save_even_while_no_session_is_loaded() {
    let mut fixture = Fixture::new();
    fixture.show(0);
    let save = fixture.pending_save(0);
    fixture.app.refresh_text();
    fixture.app.text.pending = None;
    assert!(fixture.app.text.session.is_none());
    assert!(fixture.app.text.saving.is_some());
    save.send(Ok(())).expect("save result");
    fixture.app.poll_text();
    fixture.app.poll_text();
    assert_eq!(fixture.corrections(0), 1);
    assert!(fixture.app.text.session.is_none());
}

#[test]
fn leaving_check_text_preserves_the_save_and_queues_it_when_finished() {
    let mut fixture = Fixture::new();
    fixture.show(0);
    let save = fixture.pending_save(0);
    fixture.app.show_tab(DetailTab::Overview);
    assert!(fixture.app.text.job.is_none());
    assert!(fixture.app.text.session.is_none());
    assert_eq!(fixture.app.text.parked_saves.len(), 1);
    save.send(Ok(())).expect("save result after leaving text");
    fixture.app.poll_text();
    assert_eq!((fixture.corrections(0), fixture.corrections(1)), (1, 0));
}

#[test]
fn saves_for_two_videos_finish_in_reverse_order_with_their_own_reruns() {
    let mut fixture = Fixture::new();
    fixture.show(0);
    let original = fixture.pending_save(0);
    fixture.show(1);
    let selected = fixture.pending_save(1);
    selected.send(Ok(())).expect("selected save first");
    fixture.app.poll_text();
    assert_eq!((fixture.corrections(0), fixture.corrections(1)), (0, 1));
    original.send(Ok(())).expect("original save later");
    fixture.app.poll_text();
    fixture.app.poll_text();
    assert_eq!((fixture.corrections(0), fixture.corrections(1)), (1, 1));
    fixture.assert_selected_session_unchanged(1);
}

#[test]
fn returning_to_a_video_restores_its_saving_state_without_discarding_other_saves() {
    let mut fixture = Fixture::new();
    fixture.show(0);
    let original = fixture.pending_save(0);
    fixture.show(1);
    let selected = fixture.pending_save(1);
    fixture.show(0);
    assert_eq!(
        fixture
            .app
            .text
            .saving
            .as_ref()
            .expect("original save")
            .video,
        fixture.videos[0]
    );
    assert_eq!(fixture.app.text.parked_saves.len(), 1);
    fixture.app.apply_text(Event::Save);
    assert!(
        !fixture
            .session(0)
            .work
            .join("visual/corrections.json")
            .exists(),
        "duplicate save stays disabled"
    );
    original.send(Ok(())).expect("original save");
    selected.send(Ok(())).expect("selected save");
    fixture.app.poll_text();
    assert_eq!((fixture.corrections(0), fixture.corrections(1)), (1, 1));
}

#[test]
fn failure_after_switching_names_its_owner_without_changing_the_new_session() {
    let mut fixture = Fixture::new();
    fixture.show(0);
    let save = fixture.pending_save(0);
    fixture.show(1);
    save.send(Err("Disk full".into())).expect("failed save");
    fixture.app.poll_text();
    fixture.assert_selected_session_unchanged(1);
    assert_eq!((fixture.corrections(0), fixture.corrections(1)), (0, 0));
    let toast = fixture.app.toasts.shown().last().expect("visible failure");
    assert_eq!(toast.kind, ToastKind::Error);
    assert!(toast.text.contains("original.mkv") && toast.text.contains("Disk full"));
}

#[test]
fn disconnected_save_reports_failure_once_even_when_the_editor_is_closed() {
    let mut fixture = Fixture::new();
    fixture.show(0);
    drop(fixture.pending_save(0));
    fixture.app.close_text();
    fixture.app.poll_text();
    let count = fixture.app.toasts.shown().len();
    assert!(
        fixture
            .app
            .toasts
            .shown()
            .iter()
            .any(|toast| toast.text.contains("stopped before reporting"))
    );
    fixture.app.poll_text();
    assert_eq!(fixture.app.toasts.shown().len(), count);
    assert_eq!(fixture.corrections(0), 0);
}

#[test]
fn actual_async_write_survives_navigation_and_queues_only_its_saved_video() {
    let mut fixture = Fixture::new();
    fixture.show(0);
    fixture.app.apply_text(Event::Save);
    fixture.show(1);
    let deadline = Instant::now() + Duration::from_secs(2);
    while fixture.corrections(0) == 0 && Instant::now() < deadline {
        fixture.app.poll_text();
        std::thread::sleep(Duration::from_millis(2));
    }
    let corrections: TextCorrections =
        pipeline::work_dir::read_json(&fixture.session(0).work.join("visual/corrections.json"))
            .expect("actual persisted correction");
    assert_eq!(
        corrections.edits["sign"].english.as_deref(),
        Some("English 0")
    );
    assert_eq!((fixture.corrections(0), fixture.corrections(1)), (1, 0));
    fixture.assert_selected_session_unchanged(1);
    fixture.app.poll_text();
    assert_eq!(fixture.corrections(0), 1);
}

#[test]
fn legacy_resume_and_correction_start_without_visual_models_required_only_by_new_jobs() {
    use job_model::job::{JobRecord, JobSettings, OutputFormat};
    use pipeline::work_dir::{self, WorkDir};

    for correction in [false, true] {
        let (send, started) = mpsc::channel();
        let mut fixture = Fixture::with_runner(Arc::new(move |video, options, _| {
            send.send((video.to_path_buf(), options.settings.clone()))
                .expect("started job");
            Ok(pipeline::JobOutcome {
                work_dir: options.work_root.join(work_dir::job_id(video)),
                subtitles: video.with_extension("srt"),
                report: options.work_root.join("report.md"),
                qc: Default::default(),
                ran: Vec::new(),
                skipped: Vec::new(),
            })
        }));
        assert!(fixture.app.settings.saved.onscreen_text.enabled);
        for model in &mut fixture.app.settings.items {
            model.present = !matches!(model.id.as_str(), "pp-ocrv5" | "manga-ocr" | "qwen3.5-4b");
        }
        assert_eq!(
            fixture
                .app
                .settings
                .items
                .iter()
                .filter(|model| !model.present)
                .count(),
            3
        );
        let work = WorkDir::new(
            fixture
                .root
                .join("work")
                .join(work_dir::job_id(&fixture.videos[0])),
        );
        let record = JobRecord {
            video: fixture.videos[0].to_string_lossy().into_owned(),
            video_size: 5,
            video_modified_s: 0,
            settings: JobSettings::with_glossary(Vec::new()),
            models_dir: None,
            corrections: None,
            steps: Default::default(),
        };
        let mut legacy = serde_json::to_value(record).expect("legacy record");
        legacy["settings"]
            .as_object_mut()
            .expect("settings object")
            .remove("onscreen_text");
        work_dir::write_json(&work.job_json(), &legacy)
            .expect("record from before visual translation");
        fixture
            .app
            .queue
            .get_mut(fixture.ids[0])
            .expect("legacy row")
            .keep_settings = true;
        assert!(!fixture.app.models_missing_for(fixture.ids[0]));
        assert!(
            fixture.app.models_missing_for(fixture.ids[1]),
            "new defaults still require visual models"
        );
        if correction {
            queue_editing::queue_review(&mut fixture.app.queue, fixture.videos[0].clone(), 1);
            fixture.app.start_next();
        } else {
            fixture
                .app
                .queue
                .get_mut(fixture.ids[0])
                .expect("legacy row")
                .state = JobState::Cancelled { kept_steps: 2 };
            fixture
                .app
                .apply_queue(JobQueueEvent::TryAgain(fixture.ids[0], None));
        }
        let (video, settings) = started
            .recv_timeout(Duration::from_secs(2))
            .expect("legacy job starts despite missing visual downloads");
        assert_eq!(video, fixture.videos[0]);
        assert!(!settings.onscreen_text.enabled);
        assert_eq!(settings.effective_output_format(), OutputFormat::Srt);
        assert!(started.try_recv().is_err(), "one job starts");
    }
}

#[test]
fn an_unrelated_job_completion_keeps_the_unsaved_text_draft_and_current_session() {
    let mut fixture = Fixture::new();
    fixture.show(0);
    let mut draft = fixture
        .app
        .text
        .session
        .as_ref()
        .expect("session")
        .draft
        .clone()
        .expect("draft");
    draft.english = Some("Owner wording still being edited".into());
    draft.start_s = 1.1;
    draft.presentation.font_size = Some(48.0);
    fixture.app.apply_text(Event::Edit(draft.clone()));
    fixture.app.apply_text(Event::Play);
    let (send, events) = mpsc::channel();
    fixture.app.runner.events = events;
    send.send(RunnerEvent::Ended(
        fixture.ids[1],
        Ok(pipeline::JobOutcome {
            work_dir: fixture.root.join("unrelated-work"),
            subtitles: fixture.videos[1].with_extension("ass"),
            report: fixture.root.join("report.md"),
            qc: Default::default(),
            ran: Vec::new(),
            skipped: Vec::new(),
        }),
    ))
    .expect("unrelated completion");
    crate::application::actions::runner::poll_runner(&mut fixture.app);
    assert!(matches!(
        fixture
            .app
            .queue
            .get(fixture.ids[1])
            .expect("other row")
            .state,
        JobState::Finished(_)
    ));
    assert_eq!(fixture.app.text.job, Some(fixture.ids[0]));
    assert!(
        fixture.app.text.pending.is_none(),
        "other videos must not trigger a text reload"
    );
    assert!(
        fixture.app.text.player.is_some(),
        "other videos must not replace the active preview"
    );
    let session = fixture
        .app
        .text
        .session
        .as_ref()
        .expect("session remains open");
    assert_eq!(session.video, fixture.videos[0]);
    assert_eq!(session.draft.as_ref(), Some(&draft));
}

#[test]
fn selecting_another_video_stops_its_hidden_preview_and_finishes_the_original_async_save() {
    let mut fixture = Fixture::new();
    fixture.show(0);
    fixture.app.apply_text(Event::Play);
    assert!(
        fixture.app.text.player.is_some(),
        "preview player is installed"
    );
    fixture.app.apply_text(Event::Save);
    assert!(fixture.app.text.saving.is_some());
    fixture
        .app
        .apply_queue(JobQueueEvent::Select(fixture.ids[1]));
    assert_eq!(fixture.app.queue.selected, Some(fixture.ids[1]));
    assert!(
        fixture.app.text.player.is_none(),
        "selection drops and cancels the hidden player"
    );
    assert!(fixture.app.text.session.is_none());
    assert!(fixture.app.text.job.is_none());
    let deadline = Instant::now() + Duration::from_secs(2);
    while fixture.corrections(0) == 0 && Instant::now() < deadline {
        fixture.app.poll_text();
        std::thread::sleep(Duration::from_millis(2));
    }
    fixture.app.poll_text();
    assert_eq!((fixture.corrections(0), fixture.corrections(1)), (1, 0));
    let saved: TextCorrections =
        pipeline::work_dir::read_json(&fixture.session(0).work.join("visual/corrections.json"))
            .expect("original correction persisted");
    assert_eq!(saved.edits["sign"].english.as_deref(), Some("English 0"));
    assert!(fixture.app.text.parked_saves.is_empty());
    assert_eq!(fixture.app.queue.selected, Some(fixture.ids[1]));
}

/// A localized video not written yet, whose one replacement has a replaced plate on disk.
fn localized_review(fixture: &Fixture) -> crate::text_review::models::LocalizedReview {
    use crate::text_review::models::{LocalizedReview, PreviewMode, Replacement};
    use job_model::onscreen::ReplaceStatus;
    let plate = fixture.root.join("plate.png");
    image::RgbImage::from_pixel(64, 16, image::Rgb([1, 2, 3]))
        .save(&plate)
        .expect("plate");
    LocalizedReview {
        video: None,
        subtitles: None,
        replacements: [(
            "sign".to_string(),
            Replacement {
                status: ReplaceStatus::Baked,
                preview: Some(plate),
                mask: None,
                check: None,
            },
        )]
        .into(),
        mode: PreviewMode::Localized,
        show_mask: false,
        pictures: None,
    }
}

#[test]
fn the_preview_switches_between_the_subtitles_and_the_localized_video() {
    use crate::text_review::models::PreviewMode;
    let mut fixture = Fixture::new();
    fixture.show(0);
    let review = localized_review(&fixture);
    let session = fixture.app.text.session.as_mut().expect("session");
    session.localized = Some(review);
    assert_eq!(
        player::rendered_source(session),
        None,
        "nothing to render before the localized video is written"
    );
    fixture
        .app
        .apply_text(Event::PreviewMode(PreviewMode::Subtitles));
    let session = fixture.app.text.session.as_mut().expect("session");
    assert_eq!(
        session.localized.as_ref().map(|l| l.mode),
        Some(PreviewMode::Subtitles)
    );
    assert_eq!(
        player::rendered_source(session),
        Some((session.video.clone(), Some(session.ass.clone())))
    );
    assert!(fixture.app.text.player.is_some(), "the preview restarts");
    let written = fixture.root.join("original.localized.mkv");
    let subtitles = fixture.root.join("original.localized.ass");
    let session = fixture.app.text.session.as_mut().expect("session");
    let localized = session.localized.as_mut().expect("localized");
    localized.video = Some(written.clone());
    localized.subtitles = Some(subtitles.clone());
    fixture
        .app
        .apply_text(Event::PreviewMode(PreviewMode::Localized));
    let session = fixture.app.text.session.as_ref().expect("session");
    assert_eq!(
        player::rendered_source(session),
        Some((written, Some(subtitles))),
        "the localized video plays with its own subtitle file"
    );
    fixture.app.apply_text(Event::ShowMask(true));
    let session = fixture.app.text.session.as_ref().expect("session");
    assert!(session.localized.as_ref().is_some_and(|l| l.show_mask));
}

#[test]
fn selecting_an_occurrence_loads_its_replaced_plate_off_the_window_thread() {
    let mut fixture = Fixture::new();
    fixture.show(0);
    let review = localized_review(&fixture);
    let session = fixture.app.text.session.as_mut().expect("session");
    session.localized = Some(review);
    fixture.app.apply_text(Event::Select(0));
    let deadline = Instant::now() + Duration::from_secs(5);
    let loaded = |app: &TbdSubtitlesApp| {
        app.text
            .session
            .as_ref()
            .and_then(|session| session.localized.as_ref())
            .and_then(|localized| localized.pictures.as_ref())
            .map(|pictures| (pictures.id.clone(), pictures.preview.is_some()))
    };
    while loaded(&fixture.app).is_none() && Instant::now() < deadline {
        fixture.app.poll_text();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(loaded(&fixture.app), Some(("sign".to_string(), true)));
}

#[test]
fn a_reloaded_session_keeps_the_chosen_picture_and_mask_switch() {
    use crate::text_review::models::PreviewMode;
    let mut fixture = Fixture::new();
    fixture.show(0);
    let mut review = localized_review(&fixture);
    review.mode = PreviewMode::Subtitles;
    review.show_mask = true;
    fixture
        .app
        .text
        .session
        .as_mut()
        .expect("session")
        .localized = Some(review);
    fixture.app.refresh_text();
    let mut reloaded = fixture.session(0);
    reloaded.localized = Some(localized_review(&fixture));
    let (send, receive) = mpsc::channel();
    fixture.app.text.pending = Some(receive);
    send.send(Ok(reloaded)).expect("send");
    fixture.app.poll_text();
    let session = fixture.app.text.session.as_ref().expect("reloaded");
    let localized = session.localized.as_ref().expect("localized");
    assert_eq!(
        (localized.mode, localized.show_mask),
        (PreviewMode::Subtitles, true)
    );
}
