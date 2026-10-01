use std::fs;
use std::io::Write;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::*;
use crate::model_store::manifest::PinnedArchive;

fn scratch(label: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("archive-libraries-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn file(tar: &mut tar::Builder<impl Write>, path: &str, body: &[u8]) {
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Regular);
    header.set_size(body.len() as u64);
    header.set_mode(0o755);
    tar.append_data(&mut header, path, body).unwrap();
}

fn link(tar: &mut tar::Builder<impl Write>, kind: tar::EntryType, path: &str, target: &str) {
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(kind);
    header.set_size(0);
    header.set_mode(0o777);
    tar.append_link(&mut header, path, target).unwrap();
}

/// A TensorRT-like `.tar.gz`: libraries under `lib/` and `targets/<triple>/lib/`, with soname
/// symlinks, a hard link, headers, static archives, stubs and a Windows builder resource.
fn tensorrt_like() -> Vec<u8> {
    let gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut tar = tar::Builder::new(gzip);
    let top = "TensorRT-10.14.1.48";
    file(
        &mut tar,
        &format!("{top}/lib/libnvinfer.so.10.14.1"),
        b"nvinfer",
    );
    link(
        &mut tar,
        tar::EntryType::Symlink,
        &format!("{top}/lib/libnvinfer.so.10"),
        "libnvinfer.so.10.14.1",
    );
    link(
        &mut tar,
        tar::EntryType::Symlink,
        &format!("{top}/lib/libnvinfer.so"),
        "./libnvinfer.so.10",
    );
    file(
        &mut tar,
        &format!("{top}/lib/libnvinfer_plugin.so.10.14.1"),
        b"plugin",
    );
    link(
        &mut tar,
        tar::EntryType::Link,
        &format!("{top}/lib/libnvinfer_plugin.so.10"),
        &format!("{top}/lib/libnvinfer_plugin.so.10.14.1"),
    );
    file(
        &mut tar,
        &format!("{top}/targets/x86_64-linux-gnu/lib/libnvonnxparser.so.10.14.1"),
        b"parser",
    );
    link(
        &mut tar,
        tar::EntryType::Symlink,
        &format!("{top}/targets/x86_64-linux-gnu/lib/libnvonnxparser.so.10"),
        "libnvonnxparser.so.10.14.1",
    );
    file(
        &mut tar,
        &format!("{top}/lib/libnvinfer_builder_resource.so.10.14.1"),
        b"resource",
    );
    file(
        &mut tar,
        &format!("{top}/lib/libnvinfer_builder_resource_win.so.10.14.1"),
        b"windows",
    );
    file(
        &mut tar,
        &format!("{top}/lib/libnvinfer_static.a"),
        b"static",
    );
    file(&mut tar, &format!("{top}/lib/stubs/libnvinfer.so"), b"stub");
    file(&mut tar, &format!("{top}/include/NvInfer.h"), b"header");
    file(
        &mut tar,
        &format!("{top}/doc/libnvinfer.so.txt"),
        b"not a library",
    );
    tar.into_inner().unwrap().finish().unwrap()
}

fn pinned(bytes: &[u8], sha256: Option<&str>) -> PinnedLibraries {
    let hash = sha256
        .map(str::to_string)
        .unwrap_or_else(|| hex(&Sha256::digest(bytes)));
    PinnedLibraries {
        archive: PinnedArchive {
            id: "tensorrt-test",
            unpack_to: "tensorrt-test",
            url: "https://invalid.invalid/tars/TensorRT-test.tar.gz",
            size: bytes.len() as u64,
            sha256: hash.leak(),
        },
        prefixes: &[
            "libnvinfer.so",
            "libnvinfer_plugin.so",
            "libnvonnxparser.so",
            "libnvinfer_builder_resource",
        ],
        skipped: &["_win."],
    }
}

fn keep_going(_: u64, _: u64) -> ControlFlow<()> {
    ControlFlow::Continue(())
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn library_name_takes_files_directly_in_a_lib_folder() {
    let name = |p: &str| library_name(Path::new(p));
    assert_eq!(
        name("T/lib/libnvinfer.so.10").as_deref(),
        Some("libnvinfer.so.10")
    );
    assert_eq!(
        name("T/targets/x86_64-linux-gnu/lib/libnvinfer.so.10").as_deref(),
        Some("libnvinfer.so.10")
    );
    assert_eq!(name("T/lib/stubs/libnvinfer.so"), None);
    assert_eq!(name("T/include/NvInfer.h"), None);
    assert_eq!(name("T/lib"), None);
    assert_eq!(name("T/../lib/libnvinfer.so.10"), None);
    assert_eq!(name("/lib/libnvinfer.so.10"), None);
}

#[test]
fn only_the_selected_libraries_are_installed_from_a_placed_archive() {
    let runtime = scratch("placed");
    let bytes = tensorrt_like();
    let libraries = pinned(&bytes, None);
    let placed = placed_path(&libraries, &runtime);
    fs::create_dir_all(placed.parent().unwrap()).unwrap();
    fs::write(&placed, &bytes).unwrap();

    let mut last = 0;
    let names = install(&libraries, &runtime, &mut |held, _| {
        last = held;
        ControlFlow::Continue(())
    })
    .unwrap();
    assert_eq!(last, bytes.len() as u64, "every byte is read and reported");
    let expected = [
        "libnvinfer.so",
        "libnvinfer.so.10",
        "libnvinfer.so.10.14.1",
        "libnvinfer_builder_resource.so.10.14.1",
        "libnvinfer_plugin.so.10",
        "libnvinfer_plugin.so.10.14.1",
        "libnvonnxparser.so.10",
        "libnvonnxparser.so.10.14.1",
    ];
    assert_eq!(names.len(), expected.len(), "{names:?}");
    let lib = runtime.join("tensorrt-test").join("lib");
    assert_eq!(names_in(&lib), expected);
    assert_eq!(fs::read(lib.join("libnvinfer.so")).unwrap(), b"nvinfer");
    assert_eq!(
        fs::read_link(lib.join("libnvinfer.so")).unwrap(),
        Path::new("libnvinfer.so.10")
    );
    assert_eq!(
        fs::read(lib.join("libnvinfer_plugin.so.10")).unwrap(),
        b"plugin"
    );
    assert_eq!(
        fs::read(lib.join("libnvonnxparser.so.10")).unwrap(),
        b"parser"
    );
    assert!(is_installed(&libraries, &runtime));
    assert!(
        !placed.exists(),
        "the placed archive is deleted once installed"
    );
    assert!(!runtime.join(".staging").join("tensorrt-test").exists());

    // Installed: nothing is read again.
    assert!(
        install(&libraries, &runtime, &mut keep_going)
            .unwrap()
            .is_empty()
    );
    fs::remove_dir_all(&runtime).unwrap();
}

#[test]
fn a_wrong_hash_installs_nothing_and_removes_what_was_unpacked() {
    let runtime = scratch("wrong-hash");
    let bytes = tensorrt_like();
    let libraries = pinned(&bytes, Some(&"0".repeat(64)));
    let staging = runtime.join(".staging").join("t");
    let error = unpack_verified(
        bytes.as_slice(),
        &libraries,
        &staging,
        Path::new("test.tar.gz"),
        &mut keep_going,
    )
    .unwrap_err();
    assert!(
        matches!(error, StoreError::ChecksumMismatch { .. }),
        "{error}"
    );
    assert!(!staging.exists());
    assert!(!is_installed(&libraries, &runtime));
    fs::remove_dir_all(&runtime).unwrap();
}

#[test]
fn a_short_stream_is_a_size_mismatch() {
    let runtime = scratch("short");
    let bytes = tensorrt_like();
    let mut libraries = pinned(&bytes, None);
    libraries.archive.size += 1;
    let staging = runtime.join("s");
    let error = unpack_verified(
        bytes.as_slice(),
        &libraries,
        &staging,
        Path::new("t"),
        &mut keep_going,
    )
    .unwrap_err();
    assert!(matches!(error, StoreError::SizeMismatch { .. }), "{error}");
    assert!(!staging.exists());
    fs::remove_dir_all(&runtime).unwrap();
}

#[test]
fn a_prefix_that_matches_nothing_fails_naming_it() {
    let runtime = scratch("prefix");
    let bytes = tensorrt_like();
    let mut libraries = pinned(&bytes, None);
    libraries.prefixes = &["libnvinfer.so", "libnvinfer_missing"];
    let error = unpack_verified(
        bytes.as_slice(),
        &libraries,
        &runtime.join("s"),
        Path::new("t"),
        &mut keep_going,
    )
    .unwrap_err();
    assert!(error.to_string().contains("libnvinfer_missing"), "{error}");
    fs::remove_dir_all(&runtime).unwrap();
}

#[test]
fn a_stopped_read_is_cancelled_and_leaves_nothing() {
    let runtime = scratch("cancel");
    let bytes = tensorrt_like();
    let libraries = pinned(&bytes, None);
    let staging = runtime.join("s");
    let error = unpack_verified(
        bytes.as_slice(),
        &libraries,
        &staging,
        Path::new("t"),
        &mut |_, _| ControlFlow::Break(()),
    )
    .unwrap_err();
    assert!(matches!(error, StoreError::Cancelled), "{error}");
    assert!(!staging.exists());
    fs::remove_dir_all(&runtime).unwrap();
}

#[test]
fn a_page_that_is_not_gzip_is_refused() {
    let runtime = scratch("html");
    let page = b"<html>sign in</html>".to_vec();
    let libraries = pinned(&page, None);
    let error = unpack_verified(
        page.as_slice(),
        &libraries,
        &runtime.join("s"),
        Path::new("t"),
        &mut keep_going,
    )
    .unwrap_err();
    assert!(matches!(error, StoreError::Archive { .. }), "{error}");
    fs::remove_dir_all(&runtime).unwrap();
}
