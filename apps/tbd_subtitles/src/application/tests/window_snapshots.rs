//! Snapshot scenes of the real window, rendered offscreen for visual review.
//!
//! **Role:** drive `TbdSubtitlesApp` through scripted scenes (clicks by label, light and dark,
//! fonts installed) and write one PNG per scene and scheme at 1280 by 800.
//!
//! **Position:** a child of the rendering tests; an ignored test, run on the host with
//! `TBD_SNAPSHOTS=<folder> cargo test -p tbd_subtitles -- --ignored window_snapshots`.
//!
//! **Signals and state:** reads the owner's Dressrosa 11 and 15–17 work folders and copies their
//! JSON files into a scratch folder; writes PNGs only to `$TBD_SNAPSHOTS`, never into the repo.
//!
//! **Invariants:** the owner's work folders and videos are only read; the app runs over the
//! scratch copy with a stand-in runner, and a running, failed or cancelled job is set by hand, so
//! no job starts.

use std::path::Path;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::*;

/// The work folders the scenes are built from.
const EPISODES: [&str; 4] = ["11", "15", "16", "17"];
/// Episodes queued in the running scene with no work folder: they only wait.
const WAITING: [u32; 9] = [12, 13, 14, 18, 19, 20, 21, 22, 23];
/// The files a report and a review read from a work folder.
const JOB_FILES: [&str; 7] = [
    "qc.json",
    "job.json",
    "output.json",
    "probe.json",
    "sheet.json",
    "adjudicated.json",
    "review.json",
];

#[test]
#[ignore = "renders PNGs for visual review; needs a GPU"]
fn window_snapshots() {
    let out = std::env::var_os("TBD_SNAPSHOTS")
        .map(PathBuf::from)
        .expect("TBD_SNAPSHOTS names the folder for the PNGs");
    std::fs::create_dir_all(&out).expect("the snapshot folder");
    let root = std::env::temp_dir().join(format!("tbd-snapshots-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let videos = copy_work_folders(&root.join("work"));
    finished_scenes(&root, &out, &videos);
    first_run_scene(&root.join("first-run"), &out);
    queue_scenes(&root, &out, &videos);
    detail_scenes(&root, &out, &videos);
    attention_scene(&root, &out, &videos);
    let _ = std::fs::remove_dir_all(&root);
}

/// The window over `root` with `setup` applied to the app before the first frame.
fn harness(
    root: &Path,
    setup: impl FnOnce(&mut TbdSubtitlesApp),
) -> Harness<'static, TbdSubtitlesApp> {
    let root = root.to_path_buf();
    Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .wgpu()
        .build_eframe(move |creation| {
            theme::install(&creation.egui_ctx);
            let mut app = TbdSubtitlesApp::new(Environment::scratch(&root, stand_in()), Vec::new());
            setup(&mut app);
            app
        })
}

/// Every video finished: the list, the Dressrosa 15 Overview, then with Details and Step times
/// open, its lines to check and the settings.
fn finished_scenes(root: &Path, out: &Path, videos: &[PathBuf]) {
    let mut harness = harness(root, all_finished(videos));
    shoot(&mut harness, out, "queue");
    harness.get_by_label("[Muhn Pace] Dressrosa 15").click();
    shoot(&mut harness, out, "report");
    // Step times first, while the closed Details leaves it on screen.
    harness.get_by_label("Step times").click();
    harness.run_steps(2);
    harness.get_by_label("Details").click();
    harness.run_steps(2);
    // Down to the open Details, with Step times under it.
    for _ in 0..7 {
        harness.get_by_label("Details").scroll_down();
        harness.run_steps(1);
    }
    shoot(&mut harness, out, "overview_d15");
    // The header's tab, above the lines card's button of the same name.
    harness
        .get_all_by_label("Check Lines")
        .min_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
        .expect("the header offers Check Lines")
        .click();
    shoot(&mut harness, out, "review");
    harness.get_by_label("Settings").click();
    shoot(&mut harness, out, "settings");
}

/// The setup of a window with `videos` all finished in an earlier window, their rows summed up
/// from their work folders.
fn all_finished(videos: &[PathBuf]) -> impl FnOnce(&mut TbdSubtitlesApp) + use<> {
    let videos = videos.to_vec();
    move |app| {
        app.settings.items.iter_mut().for_each(|i| i.present = true);
        app.apply(vec![Action::QueueVideos(videos)]);
        for item in &mut app.queue.items {
            item.state = JobState::FinishedBefore;
        }
        app.refresh_summaries(None);
    }
}

/// Every video finished, Dressrosa 16 with a failed language-model call added to its check: the
/// needs-attention Overview. It changes the scratch copy of 16's `qc.json`, so it runs last.
fn attention_scene(root: &Path, out: &Path, videos: &[PathBuf]) {
    let qc_path = std::fs::read_dir(root.join("work"))
        .expect("the scratch work folder")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name().is_some_and(|name| {
                name.to_string_lossy()
                    .starts_with("muhn-pace-dressrosa-16-")
            })
        })
        .expect("Dressrosa 16's work folder")
        .join("qc.json");
    let mut qc: QcReport =
        serde_json::from_str(&std::fs::read_to_string(&qc_path).expect("qc.json"))
            .expect("qc.json parses");
    qc.findings.push(job_model::report::QcFinding {
        check: job_model::report::QcCheck::FailedCall,
        time_s: 0.0,
        text: String::new(),
        detail: "batch 7 of 12: the model answered with no JSON".into(),
        utterance: None,
    });
    std::fs::write(&qc_path, serde_json::to_string(&qc).expect("json")).expect("qc.json");
    let _ = std::fs::remove_dir_all(root.join("data"));
    let mut harness = harness(root, all_finished(videos));
    harness.get_by_label("[Muhn Pace] Dressrosa 16").click();
    shoot(&mut harness, out, "needs_attention");
}

/// The first window: no videos, the models still missing.
fn first_run_scene(root: &Path, out: &Path) {
    let mut harness = harness(root, |_| {});
    shoot(&mut harness, out, "first_run");
}

/// A running queue: Dressrosa 16 settling its words, the rest waiting, 15 and 11 done; then a
/// waiting row's menu, and the toast after it is removed.
fn queue_scenes(root: &Path, out: &Path, videos: &[PathBuf]) {
    let queued = queued_videos(root, videos);
    let mut harness = harness(root, move |app| {
        running_queue(app, queued);
        app.queue.selected = id_of(app, "15");
        app.refresh_report(false);
    });
    shoot(&mut harness, out, "running_queue");
    harness
        .get_by_label("[Muhn Pace] Dressrosa 17")
        .click_secondary();
    shoot(&mut harness, out, "row_menu");
    // The menu's command, left of the selected row's card, which offers it too.
    harness
        .get_all_by_label("Remove from List")
        .min_by(|a, b| a.rect().left().total_cmp(&b.rect().left()))
        .expect("the menu offers Remove from List")
        .click();
    shoot(&mut harness, out, "undo_toast");
}

/// The detail pane of the running queue: Dressrosa 16 running, 17 next in line, 19 failed while
/// hearing the speech and 20 cancelled.
fn detail_scenes(root: &Path, out: &Path, videos: &[PathBuf]) {
    let queued = queued_videos(root, videos);
    let mut harness = harness(root, move |app| {
        running_queue(app, queued);
        if let Some(item) = id_of(app, "19").and_then(|id| app.queue.get_mut(id)) {
            let finished = [
                (StepName::ProbeDecode, 7.5),
                (StepName::ShotScan, 41.0),
                (StepName::Separation, 190.0),
                (StepName::Vad, 1.1),
                (StepName::AsrParakeet, 16.0),
            ]
            .map(|(step, seconds)| (step, FinishedStep::Done(Some(seconds))))
            .to_vec();
            item.state = JobState::Failed(Failure::new(
                Some(StepName::AsrWhisper),
                "the Whisper worker stopped early (exit status 1); its log is \
                 logs/asr_whisper.log"
                    .into(),
                finished,
            ));
        }
        if let Some(item) = id_of(app, "20").and_then(|id| app.queue.get_mut(id)) {
            item.state = JobState::Cancelled { kept_steps: 9 };
        }
    });
    for (episode, scene) in [
        ("16", "running_detail"),
        ("17", "waiting_card"),
        ("19", "failed_card"),
        ("20", "cancelled_card"),
    ] {
        let id = id_of(harness.state(), episode).expect("the episode is queued");
        harness
            .state_mut()
            .apply(vec![Action::from(JobQueueEvent::Select(id))]);
        shoot(&mut harness, out, scene);
    }
}

/// The finished videos and the waiting episodes, in a queue of their own rather than the one the
/// finished scenes kept.
fn queued_videos(root: &Path, videos: &[PathBuf]) -> Vec<PathBuf> {
    let _ = std::fs::remove_dir_all(root.join("data"));
    let mut queued = videos.to_vec();
    let folder = root.join("videos");
    queued.extend(
        WAITING
            .iter()
            .map(|n| folder.join(format!("[Muhn Pace] Dressrosa {n}.mp4"))),
    );
    queued
}

/// Queue `queued` with the models on disk, 11 and 15 finished, 16 settling its words, and the
/// queue running.
fn running_queue(app: &mut TbdSubtitlesApp, queued: Vec<PathBuf>) {
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.apply(vec![Action::QueueVideos(queued)]);
    for episode in ["11", "15"] {
        if let Some(item) = id_of(app, episode).and_then(|id| app.queue.get_mut(id)) {
            item.state = JobState::FinishedBefore;
        }
    }
    let running = id_of(app, "16");
    if let Some(item) = running.and_then(|id| app.queue.get_mut(id)) {
        item.state = JobState::Running(Box::new(settling(Instant::now())));
    }
    app.queue.running = true;
    // The full lane holds the running job, with no thread behind it.
    app.cancel = running.map(|id| (id, CancelToken::new()));
    app.refresh_summaries(None);
}

/// The job of Dressrosa `episode`.
fn id_of(app: &TbdSubtitlesApp, episode: &str) -> Option<JobId> {
    let name = format!("[Muhn Pace] Dressrosa {episode}");
    app.queue
        .items
        .iter()
        .find(|item| item.name() == name)
        .map(|item| item.id)
}

/// Render the scene in light and in dark, as `<scene>_light.png` and `<scene>_dark.png`.
fn shoot(harness: &mut Harness<'_, TbdSubtitlesApp>, out: &Path, scene: &str) {
    for (scheme, name) in [(Scheme::Light, "light"), (Scheme::Dark, "dark")] {
        harness.state_mut().scheme = scheme;
        // A spinner asks for frames without end; the scene is taken where it stands.
        let _ = harness.run_ok();
        let image = harness
            .render()
            .unwrap_or_else(|error| panic!("{scene} in {name} did not render: {error}"));
        let path = out.join(format!("{scene}_{name}.png"));
        image
            .save(&path)
            .unwrap_or_else(|error| panic!("{} was not written: {error}", path.display()));
    }
}

/// Copy the JSON files of the owner's work folders into `work`, keeping each folder's name, and
/// return the videos they belong to.
fn copy_work_folders(work: &Path) -> Vec<PathBuf> {
    let source = inference::model_store::app_data_dir()
        .expect("the app data folder")
        .join("work");
    let mut videos = Vec::new();
    for episode in EPISODES {
        let prefix = format!("muhn-pace-dressrosa-{episode}-");
        let folder = std::fs::read_dir(&source)
            .unwrap_or_else(|error| panic!("{} cannot be read: {error}", source.display()))
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with(&prefix))
            })
            .unwrap_or_else(|| panic!("no work folder {prefix}* in {}", source.display()));
        let copy = work.join(folder.file_name().expect("a folder name"));
        copy_json(&folder, &copy, &JOB_FILES);
        let adjudication = folder.join("adjudication");
        let names: Vec<String> = std::fs::read_dir(&adjudication)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .filter(|name| name.ends_with(".json"))
                    .collect()
            })
            .unwrap_or_default();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        copy_json(&adjudication, &copy.join("adjudication"), &names);
        let record: job_model::job::JobRecord = serde_json::from_str(
            &std::fs::read_to_string(copy.join("job.json")).expect("job.json"),
        )
        .expect("job.json parses");
        videos.push(PathBuf::from(record.video));
    }
    videos
}

/// Copy each of `names` that exists in `from` into `to`.
fn copy_json(from: &Path, to: &Path, names: &[&str]) {
    std::fs::create_dir_all(to).expect("a scratch folder");
    for name in names {
        let file = from.join(name);
        if file.is_file() {
            std::fs::copy(&file, to.join(name)).expect("a copied file");
        }
    }
}
