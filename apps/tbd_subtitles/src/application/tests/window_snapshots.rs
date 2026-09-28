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
//! scratch copy with a stand-in runner, and a running job is set by hand, so no job starts.

use std::path::Path;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::*;
use crate::job_queue::models::progress::StepState;

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

/// Every video finished: the list, the Dressrosa 15 report, its review and the settings.
fn finished_scenes(root: &Path, out: &Path, videos: &[PathBuf]) {
    let videos = videos.to_vec();
    let mut harness = harness(root, move |app| {
        app.settings.items.iter_mut().for_each(|i| i.present = true);
        app.apply(vec![Action::QueueVideos(videos)]);
        for item in &mut app.queue.items {
            item.state = JobState::FinishedBefore;
        }
    });
    shoot(&mut harness, out, "queue");
    harness.get_by_label("[Muhn Pace] Dressrosa 15").click();
    shoot(&mut harness, out, "report");
    harness.get_by_label("Review lines").click();
    shoot(&mut harness, out, "review");
    harness.get_by_label("Settings").click();
    shoot(&mut harness, out, "settings");
}

/// The first window: no videos, the models still missing.
fn first_run_scene(root: &Path, out: &Path) {
    let mut harness = harness(root, |_| {});
    shoot(&mut harness, out, "first_run");
}

/// A running queue: Dressrosa 16 settling its words, the rest waiting, 15 and 11 done; then a
/// waiting row's menu, and the toast after it is removed.
fn queue_scenes(root: &Path, out: &Path, videos: &[PathBuf]) {
    // A queue of its own, not the one the finished scenes kept.
    let _ = std::fs::remove_dir_all(root.join("data"));
    let mut queued = videos.to_vec();
    let folder = root.join("videos");
    queued.extend(
        WAITING
            .iter()
            .map(|n| folder.join(format!("[Muhn Pace] Dressrosa {n}.mp4"))),
    );
    let mut harness = harness(root, move |app| {
        app.settings.items.iter_mut().for_each(|i| i.present = true);
        app.apply(vec![Action::QueueVideos(queued)]);
        let now = Instant::now();
        let id_of = |app: &TbdSubtitlesApp, episode: &str| {
            let name = format!("[Muhn Pace] Dressrosa {episode}");
            app.queue
                .items
                .iter()
                .find(|item| item.name() == name)
                .map(|item| item.id)
        };
        for episode in ["11", "15"] {
            if let Some(item) = id_of(app, episode).and_then(|id| app.queue.get_mut(id)) {
                item.state = JobState::FinishedBefore;
            }
        }
        let running = id_of(app, "16");
        if let Some(item) = running.and_then(|id| app.queue.get_mut(id)) {
            item.state = JobState::Running(Box::new(settling(now)));
        }
        app.queue.running = true;
        // The full lane holds the running job, with no thread behind it.
        app.cancel = running.map(|id| (id, CancelToken::new()));
        app.queue.selected = id_of(app, "15");
        app.refresh_report(false);
    });
    shoot(&mut harness, out, "running_queue");
    harness
        .get_by_label("[Muhn Pace] Dressrosa 17")
        .click_secondary();
    shoot(&mut harness, out, "row_menu");
    harness.get_by_label("Remove from List").click();
    shoot(&mut harness, out, "undo_toast");
}

/// A job ten minutes in, its words being settled: every step before done, the rest to run.
fn settling(now: Instant) -> JobProgress {
    let mut progress = JobProgress::new(now - Duration::from_secs(600));
    progress.duration_s = Some(1559.0);
    for row in &mut progress.steps {
        row.state = match row.step {
            StepName::Adjudicate => StepState::Running {
                started: now - Duration::from_secs(40),
                done: 3,
                total: 8,
                message: Some("batch 4 of 8".into()),
            },
            step if step < StepName::Adjudicate => StepState::Done { wall_s: 30.0 },
            _ => StepState::Pending,
        };
    }
    progress
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
