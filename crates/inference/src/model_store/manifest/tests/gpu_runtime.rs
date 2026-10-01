use super::*;

fn is_hash(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[test]
fn tensorrt_is_nvidias_10_14_tarball_for_cuda_13() {
    assert!(
        TENSORRT_ARCHIVE.url.starts_with(
            "https://developer.nvidia.com/downloads/compute/machine-learning/tensorrt/"
        )
    );
    assert!(TENSORRT_ARCHIVE.url.ends_with(".tar.gz"));
    assert!(TENSORRT_ARCHIVE.url.contains(&format!(
        "TensorRT-{TENSORRT_VERSION}.Linux.x86_64-gnu.cuda-13.0"
    )));
    assert!(is_hash(TENSORRT_ARCHIVE.sha256));
    assert_eq!(TENSORRT_ARCHIVE.size, 7_004_299_092);
    assert_eq!(TENSORRT_ARCHIVE.unpack_to, TENSORRT_FOLDER);
    assert_eq!(TENSORRT_LIBRARIES.archive.sha256, TENSORRT_ARCHIVE.sha256);
}

#[test]
fn the_tensorrt_folder_names_the_major_and_minor_version() {
    let mut parts = TENSORRT_VERSION.split('.');
    let major = parts.next().unwrap();
    let minor = parts.next().unwrap();
    assert_eq!(major, "10", "the provider links libnvinfer.so.10");
    assert_eq!(TENSORRT_FOLDER, format!("tensorrt-{major}.{minor}"));
}

#[test]
fn the_tensorrt_selection_keeps_what_the_provider_loads() {
    for kept in [
        "libnvinfer.so",
        "libnvinfer.so.10",
        "libnvinfer.so.10.14.1",
        "libnvinfer_plugin.so.10",
        "libnvonnxparser.so.10.14.1",
        "libnvinfer_builder_resource.so.10.14.1",
        "libnvinfer_builder_resource_sm86.so.10.14.1",
    ] {
        assert!(TENSORRT_LIBRARIES.selects(kept), "{kept}");
    }
    for left in [
        "libnvinfer_static.a",
        "libnvinfer_lean.so.10",
        "libnvinfer_dispatch.so.10",
        "libnvinfer_vc_plugin.so.10",
        "libnvinfer_builder_resource_win.so.10.14.1",
        "libnvonnxparser_static.a",
    ] {
        assert!(!TENSORRT_LIBRARIES.selects(left), "{left}");
    }
}

#[test]
fn tensorrt_is_not_a_settings_download() {
    assert!(runtime_archives().all(|archive| archive.id != TENSORRT_ARCHIVE.id));
}

#[test]
fn the_marker_changes_with_the_selection() {
    let narrower = PinnedLibraries {
        prefixes: &["libnvinfer.so"],
        ..TENSORRT_LIBRARIES
    };
    assert!(
        TENSORRT_LIBRARIES
            .marker()
            .starts_with(TENSORRT_ARCHIVE.sha256)
    );
    assert_ne!(narrower.marker(), TENSORRT_LIBRARIES.marker());
}
