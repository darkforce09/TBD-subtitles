use super::*;
use crate::settings::models::app_settings::AppSettings;

fn empty_folders(name: &str) -> Folders {
    let root = std::env::temp_dir().join(format!("tbd-downloads-{name}-{}", std::process::id()));
    Folders {
        models: root.join("models"),
        runtime: root.join("runtime"),
        exe_dir: None,
    }
}

fn item(kind: ItemKind, id: &str, bytes: u64) -> DownloadItem {
    DownloadItem {
        kind,
        id: id.into(),
        bytes,
        present: false,
    }
}

fn page(items: Vec<DownloadItem>) -> SettingsPage {
    SettingsPage {
        path: PathBuf::from("/nowhere/settings.toml"),
        saved: AppSettings::default(),
        error: None,
        unreadable: None,
        glossary_names: None,
        items,
        download: None,
        downloaded_at: None,
        checks: None,
        checking: false,
        models_folder: PathBuf::from("/models"),
        models_size: None,
        work_folder: PathBuf::from("/work"),
        work_size: None,
        right_click: Default::default(),
        library: Default::default(),
    }
}

#[test]
fn empty_folders_miss_every_model_and_runtime_archive() {
    let settings = JobSettings::with_glossary(vec![]);
    let items = plan(&empty_folders("empty"), &settings);
    let models: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == ItemKind::Model)
        .map(|i| i.id.as_str())
        .collect();
    assert_eq!(models, pipeline::models::required(&settings));
    assert!(items.iter().all(|i| !i.present));
    assert!(items.iter().all(|i| i.bytes > 0));
    let progress = begun(&items).expect("something to download");
    assert_eq!(progress.id, models[0]);
    assert_eq!(progress.size, items.iter().map(|i| i.bytes).sum::<u64>());
}

#[test]
fn replacing_text_in_the_video_lists_lama_and_the_latin_fonts_with_their_sizes() {
    let mut settings = JobSettings::with_glossary(vec![]);
    settings.onscreen_text.enabled = true;
    settings.onscreen_text.localized_video = false;
    let folders = empty_folders("localized");
    let without = plan(&folders, &settings);
    assert!(
        !without
            .iter()
            .any(|i| i.id == "lama-inpaint" || i.id == "latin-fonts")
    );
    settings.onscreen_text.localized_video = true;
    let with = plan(&folders, &settings);
    let size = |id: &str| with.iter().find(|i| i.id == id).map(|i| i.bytes);
    assert_eq!(size("lama-inpaint"), Some(208_044_816));
    assert_eq!(size("latin-fonts"), Some(2_049_096 + 4_396));
    let rows = crate::settings::services::model_list::rows(&with, None);
    assert!(rows.iter().any(|row| row.name == "lama-inpaint"));
    assert!(rows.iter().any(|row| row.name == "latin-fonts"));
}

#[test]
fn the_build_toolkit_is_not_downloaded_for_the_app() {
    let ids: Vec<&str> = runtime_archives().map(|a| a.id).collect();
    assert!(ids.contains(&"onnxruntime"));
    assert!(ids.contains(&"cuda_cudart"));
    for build in model_store::manifest::CUDA_BUILD_ARCHIVES {
        assert!(
            !runtime_archives().any(|a| std::ptr::eq(a, build)),
            "{}",
            build.id
        );
    }
}

#[test]
fn a_download_starts_at_the_first_missing_item_and_counts_only_the_missing() {
    let mut items = vec![
        item(ItemKind::Model, "a", 10),
        item(ItemKind::Runtime, "b", 5),
    ];
    items[0].present = true;
    let progress = begun(&items).expect("b is missing");
    assert_eq!((progress.id.as_str(), progress.size), ("b", 5));
    items[1].present = true;
    assert_eq!(begun(&items), None, "nothing to download");
}

#[test]
fn a_list_planned_again_during_a_download_is_marked_by_id() {
    let mut page = page(vec![
        item(ItemKind::Model, "whisper-large-v3", 30),
        item(ItemKind::Model, "ced-base", 10),
    ]);
    page.download = begun(&page.items);
    let advanced = DownloadEvent::Advanced {
        id: "whisper-large-v3".into(),
        held: 12,
        total: 30,
    };
    assert_eq!(fold(&mut page, advanced), None);
    // The owner picks the turbo model: the list is planned again, in another order.
    page.items = vec![
        item(ItemKind::Model, "whisper-large-v3-turbo", 20),
        item(ItemKind::Model, "ced-base", 10),
        item(ItemKind::Model, "whisper-large-v3", 30),
    ];
    let finished = DownloadEvent::Finished {
        id: "whisper-large-v3".into(),
        bytes: 30,
    };
    assert_eq!(fold(&mut page, finished), None);
    let present: Vec<(&str, bool)> = page
        .items
        .iter()
        .map(|i| (i.id.as_str(), i.present))
        .collect();
    assert_eq!(
        present,
        [
            ("whisper-large-v3-turbo", false),
            ("ced-base", false),
            ("whisper-large-v3", true),
        ]
    );
    let advanced = DownloadEvent::Advanced {
        id: "ced-base".into(),
        held: 4,
        total: 10,
    };
    fold(&mut page, advanced);
    let progress = page.download.clone().expect("running");
    assert_eq!(progress.id, "ced-base");
    assert_eq!((progress.done(), progress.size), (34, 40));
    assert_eq!(
        fold(&mut page, DownloadEvent::Ended(DownloadEnd::Stopped)),
        Some(DownloadEnd::Stopped)
    );
}

#[test]
fn nothing_to_download_ends_at_once() {
    let downloading = start(
        Vec::new(),
        empty_folders("none"),
        crate::core::background::no_wake(),
    );
    let event = downloading
        .events
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("event");
    assert_eq!(event, DownloadEvent::Ended(DownloadEnd::Done));
}
