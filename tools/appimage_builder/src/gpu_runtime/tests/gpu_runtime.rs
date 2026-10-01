use std::fs;
use std::path::{Path, PathBuf};

use super::*;
use crate::elf::tests::tiny_library;

/// A fresh, empty folder under the system temp folder.
fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gpu_runtime-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn library(dir: &Path, name: &str, needed: &[&str]) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join(name), tiny_library(name, needed, "")).unwrap();
}

/// A runtime folder of tiny ELF libraries with every folder's required libraries, ONNX Runtime's
/// TensorRT provider and the TensorRT libraries it links and opens.
fn fake_runtime(runtime: &Path) {
    let lib = |folder: &str| runtime.join(folder).join("lib");
    for name in REQUIRED_CUDA_LIBS {
        library(&lib(CUDA_FOLDER), name, &["libc.so.6"]);
    }
    for name in REQUIRED_CUDNN_LIBS {
        library(&lib(CUDNN_FOLDER), name, &["libstdc++.so.6"]);
    }
    for name in REQUIRED_ONNX_RUNTIME_LIBS {
        library(&lib(ONNX_RUNTIME_FOLDER), name, &[]);
    }
    library(
        &lib(ONNX_RUNTIME_FOLDER),
        TENSORRT_PROVIDER_LIB,
        &[
            "libnvinfer.so.10",
            "libnvonnxparser.so.10",
            "libonnxruntime_providers_shared.so",
            "libcudart.so.13",
        ],
    );
    let trt = lib(TENSORRT_FOLDER);
    library(&trt, "libnvinfer.so.10.14.1", &["libm.so.6"]);
    std::os::unix::fs::symlink("libnvinfer.so.10.14.1", trt.join("libnvinfer.so.10")).unwrap();
    library(&trt, "libnvonnxparser.so.10", &["libnvinfer.so.10"]);
    library(&trt, "libnvinfer_plugin.so.10", &["libnvinfer.so.10"]);
    library(&trt, "libnvinfer_builder_resource.so.10.14.1", &[]);
    library(&trt, "libnvinfer_lean.so.10", &[]);
}

#[test]
fn the_folders_follow_the_locator_and_end_with_tensorrt() {
    let names: Vec<&str> = FOLDERS.iter().map(|f| f.name).collect();
    assert_eq!(
        names,
        [
            CUDA_FOLDER,
            CUDNN_FOLDER,
            ONNX_RUNTIME_FOLDER,
            TENSORRT_FOLDER
        ]
    );
    let tensorrt = &FOLDERS[3];
    assert_eq!(
        tensorrt.required,
        ["libnvinfer.so.10", "libnvonnxparser.so.10"]
    );
    assert_eq!(
        tensorrt.loaded_at_run_time,
        ["libnvinfer_plugin.so.", "libnvinfer_builder_resource"]
    );
    assert_eq!(FOLDERS[2].providers, [TENSORRT_PROVIDER_LIB]);
}

#[test]
fn the_bundle_carries_tensorrt_and_its_provider() {
    let dir = scratch("bundle");
    let runtime = dir.join("runtime");
    let usr_bin = dir.join("AppDir/usr/bin");
    fake_runtime(&runtime);
    let bundled = bundle(&runtime, &usr_bin).unwrap();
    for name in [
        "libnvinfer.so.10",
        "libnvinfer.so.10.14.1",
        "libnvonnxparser.so.10",
        "libnvinfer_plugin.so.10",
        "libnvinfer_builder_resource.so.10.14.1",
        TENSORRT_PROVIDER_LIB,
        "libcudart.so.13",
    ] {
        assert!(bundled.contains(name), "{name} in {bundled:?}");
    }
    assert!(!bundled.contains("libnvinfer_lean.so.10"));
    let trt = usr_bin.join("cuda").join(TENSORRT_FOLDER).join("lib");
    assert!(
        fs::symlink_metadata(trt.join("libnvinfer.so.10"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(
        usr_bin
            .join("cuda")
            .join(ONNX_RUNTIME_FOLDER)
            .join("lib")
            .join(TENSORRT_PROVIDER_LIB)
            .is_file()
    );
    let packaged = CudaRuntime::locate(Some(&usr_bin), Path::new("/nonexistent")).unwrap();
    assert_eq!(
        packaged.tensorrt_root,
        Some(usr_bin.join("cuda").join(TENSORRT_FOLDER))
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_bundle_without_the_plugin_library_fails_naming_it() {
    let dir = scratch("no-plugin");
    let runtime = dir.join("runtime");
    fake_runtime(&runtime);
    fs::remove_file(
        runtime
            .join(TENSORRT_FOLDER)
            .join("lib")
            .join("libnvinfer_plugin.so.10"),
    )
    .unwrap();
    let error = bundle(&runtime, &dir.join("usr/bin")).unwrap_err();
    assert!(
        error.to_string().contains("libnvinfer_plugin.so."),
        "{error}"
    );
    fs::remove_dir_all(&dir).unwrap();
}
