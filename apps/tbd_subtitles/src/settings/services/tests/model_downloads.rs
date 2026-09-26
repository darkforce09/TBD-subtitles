use super::*;

fn empty_folders(name: &str) -> Folders {
    let root = std::env::temp_dir().join(format!("tbd-downloads-{name}-{}", std::process::id()));
    Folders {
        models: root.join("models"),
        runtime: root.join("runtime"),
        exe_dir: None,
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
    assert_eq!(
        missing_bytes(&items),
        items.iter().map(|i| i.bytes).sum::<u64>()
    );
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
fn present_items_cost_nothing_to_download() {
    let items = vec![
        DownloadItem {
            kind: ItemKind::Model,
            id: "a".into(),
            bytes: 10,
            present: true,
        },
        DownloadItem {
            kind: ItemKind::Runtime,
            id: "b".into(),
            bytes: 5,
            present: false,
        },
    ];
    assert_eq!(missing_bytes(&items), 5);
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
    assert_eq!(event, DownloadEvent::Ended(Ok(())));
}
