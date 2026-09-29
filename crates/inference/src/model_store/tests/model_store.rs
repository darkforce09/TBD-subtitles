use std::path::Path;

use super::manifest::{self, HUGGING_FACE};
use super::*;

fn is_hash(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[test]
fn every_pin_is_https_with_a_sha256() {
    for file in MODEL_FILES {
        assert!(
            file.url.starts_with(HUGGING_FACE)
                || file
                    .url
                    .starts_with("https://github.com/GreatV/oar-ocr/releases/download/")
                || file
                    .url
                    .starts_with("https://raw.githubusercontent.com/google/fonts/"),
            "{}",
            file.url
        );
        assert!(is_hash(file.sha256), "{}", file.file);
        assert!(file.size > 0);
    }
    for archive in CUDA_ARCHIVES {
        assert!(
            archive
                .url
                .starts_with("https://developer.download.nvidia.com/")
        );
        assert!(archive.url.ends_with(".tar.xz"));
        assert!(is_hash(archive.sha256), "{}", archive.id);
    }
}

#[test]
fn the_onnx_runtime_comes_from_microsoft_as_a_tgz() {
    assert!(
        ONNX_RUNTIME_ARCHIVE
            .url
            .starts_with("https://github.com/microsoft/onnxruntime/")
    );
    assert!(ONNX_RUNTIME_ARCHIVE.url.ends_with(".tgz"));
    assert!(is_hash(ONNX_RUNTIME_ARCHIVE.sha256));
    assert_eq!(
        runtime_archives().count(),
        CUDA_ARCHIVES.len() + 1 + manifest::CUDA_BUILD_ARCHIVES.len()
    );
}

#[test]
fn model_ids_are_listed_once_in_order() {
    let ids = manifest::model_ids();
    assert_eq!(ids.first(), Some(&"visual-font"));
    assert!(ids.contains(&"pp-ocrv5"));
    assert!(ids.contains(&"manga-ocr"));
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len());
}

#[test]
fn an_unknown_model_is_refused() {
    assert!(matches!(
        model_dir(Path::new("/m"), "nope"),
        Err(StoreError::UnknownModel(_))
    ));
}

#[test]
fn strip_first_drops_the_top_folder_and_refuses_escapes() {
    assert_eq!(
        strip_first(Path::new("cuda_cudart-archive/lib/libcudart.so.13")),
        Some(Path::new("lib/libcudart.so.13").to_path_buf())
    );
    assert_eq!(strip_first(Path::new("top/")), None);
    assert_eq!(strip_first(Path::new("top/../etc/passwd")), None);
    assert_eq!(strip_first(Path::new("/abs/path")), None);
}

#[test]
fn hex_is_lowercase_and_padded() {
    assert_eq!(hex(&[0x00, 0x0f, 0xab]), "000fab");
}

#[test]
fn a_file_already_in_place_is_hashed_and_kept() {
    let dir = std::env::temp_dir().join(format!("model-store-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("f.bin");
    std::fs::write(&path, b"abc").unwrap();
    assert_eq!(
        sha256_of(&path).unwrap(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    // Size matches, so no network request is made.
    fetch_verified(
        "https://invalid.invalid/",
        &path,
        3,
        "unused",
        &mut |_, _| std::ops::ControlFlow::Continue(()),
    )
    .unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}
