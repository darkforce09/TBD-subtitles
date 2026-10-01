use std::path::PathBuf;

use super::*;
use crate::settings::models::app_settings::AppSettings;

fn item(kind: ItemKind, id: &str, mib: u64, present: bool) -> DownloadItem {
    DownloadItem {
        kind,
        id: id.into(),
        bytes: mib * 1_048_576,
        present,
    }
}

/// Two models, three CUDA archives and ONNX Runtime.
fn items() -> Vec<DownloadItem> {
    vec![
        item(ItemKind::Model, "parakeet-tdt-0.6b-v2", 2048, false),
        item(ItemKind::Model, "ced-base", 1024, true),
        item(ItemKind::Runtime, "cuda_cudart", 512, false),
        item(ItemKind::Runtime, "cuda_nvrtc", 256, false),
        item(ItemKind::Runtime, "cudnn", 256, true),
        item(ItemKind::Runtime, "onnxruntime", 1024, false),
    ]
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
fn the_cuda_archives_are_one_row_after_the_models() {
    let rows = rows(&items(), None);
    let names: Vec<(&str, RowState)> = rows.iter().map(|r| (r.name.as_str(), r.state)).collect();
    assert_eq!(
        names,
        [
            ("parakeet-tdt-0.6b-v2", RowState::Missing),
            ("ced-base", RowState::OnDisk),
            ("CUDA and cuDNN (3 archives)", RowState::Missing),
            ("onnxruntime", RowState::Missing),
        ]
    );
    assert_eq!(rows[2].bytes, 1024 * 1_048_576);
}

#[test]
fn the_row_being_downloaded_shows_its_share() {
    let items = items();
    let progress = DownloadProgress {
        id: "cuda_nvrtc".into(),
        held: 128 * 1_048_576,
        total: 256 * 1_048_576,
        finished: 0,
        size: 0,
    };
    let rows = rows(&items, Some(&progress));
    // cudnn (256 MiB) is on disk and half of cuda_nvrtc: 384 of 1024 MiB.
    assert_eq!(rows[2].state, RowState::Downloading(0.375));
    assert_eq!(rows[0].state, RowState::Missing);
}

#[test]
fn missing_names_models_and_runtime_libraries_apart() {
    let missing = missing(&items());
    assert_eq!((missing.models, missing.runtime), (1, 2));
    assert_eq!(
        missing.headline(),
        "1 model and 2 runtime libraries are missing (3.8 GiB)"
    );
    let only_models = Missing {
        models: 5,
        runtime: 0,
        bytes: 11 * 1024 * 1_048_576,
    };
    assert_eq!(only_models.headline(), "5 models are missing (11.0 GiB)");
    let one_library = Missing {
        models: 0,
        runtime: 1,
        bytes: 1_048_576,
    };
    assert_eq!(
        one_library.headline(),
        "1 runtime library is missing (1.0 MiB)"
    );
}

#[test]
fn the_banner_shows_the_download_then_what_is_missing_then_briefly_that_all_is_on_disk() {
    let now = Instant::now();
    let mut page = page(items());
    assert!(matches!(banner(&page, now), Some(Banner::Missing(_))));
    page.download = crate::settings::services::model_downloads::begun(&page.items);
    assert!(matches!(banner(&page, now), Some(Banner::Downloading(_))));
    page.download = None;
    page.items.iter_mut().for_each(|i| i.present = true);
    assert_eq!(banner(&page, now), None, "no download ended here");
    page.downloaded_at = Some(now);
    assert_eq!(
        banner(&page, now),
        Some(Banner::AllOnDisk {
            until: now + ALL_ON_DISK_SHOWN
        })
    );
    assert_eq!(banner(&page, now + ALL_ON_DISK_SHOWN), None);
}
