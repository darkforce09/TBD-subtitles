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
//! scratch copy with a stand-in runner, so no job starts.

use std::path::Path;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::*;

/// The work folders the scenes are built from.
const EPISODES: [&str; 4] = ["11", "15", "16", "17"];
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
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .wgpu()
        .build_eframe(|creation| {
            theme::install(&creation.egui_ctx);
            let mut app = TbdSubtitlesApp::new(Environment::scratch(&root, stand_in()), Vec::new());
            app.settings.items.iter_mut().for_each(|i| i.present = true);
            app.apply(vec![Action::QueueVideos(videos)]);
            for item in &mut app.queue.items {
                item.state = JobState::FinishedBefore;
            }
            app
        });
    shoot(&mut harness, &out, "queue");
    harness.get_by_label("[Muhn Pace] Dressrosa 15.mp4").click();
    shoot(&mut harness, &out, "report");
    harness.get_by_label("Review lines").click();
    shoot(&mut harness, &out, "review");
    harness.get_by_label("Settings").click();
    shoot(&mut harness, &out, "settings");
    drop(harness);
    let _ = std::fs::remove_dir_all(&root);
}

/// Render the scene in light and in dark, as `<scene>_light.png` and `<scene>_dark.png`.
fn shoot(harness: &mut Harness<'_, TbdSubtitlesApp>, out: &Path, scene: &str) {
    for (scheme, name) in [(Scheme::Light, "light"), (Scheme::Dark, "dark")] {
        harness.state_mut().scheme = scheme;
        harness.run();
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
