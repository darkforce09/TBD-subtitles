use std::path::Path;

use super::*;
use crate::model_store::manifest::{ONNX_RUNTIME_FOLDER, TENSORRT_FOLDER};

fn fake_runtime(base: &Path) {
    let cuda = base.join(CUDA_FOLDER).join("lib");
    let cudnn = base.join(CUDNN_FOLDER).join("lib");
    std::fs::create_dir_all(&cuda).unwrap();
    std::fs::create_dir_all(&cudnn).unwrap();
    for lib in REQUIRED_CUDA_LIBS {
        std::fs::write(cuda.join(lib), b"").unwrap();
    }
    for lib in REQUIRED_CUDNN_LIBS {
        std::fs::write(cudnn.join(lib), b"").unwrap();
    }
    let ort = base.join(ONNX_RUNTIME_FOLDER).join("lib");
    std::fs::create_dir_all(&ort).unwrap();
    for lib in REQUIRED_ONNX_RUNTIME_LIBS {
        std::fs::write(ort.join(lib), b"").unwrap();
    }
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("cuda-runtime-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_complete_user_folder_is_found() {
    let dir = scratch("user");
    fake_runtime(&dir);
    let runtime = CudaRuntime::locate(None, &dir).unwrap();
    assert_eq!(runtime.cuda_root, dir.join(CUDA_FOLDER));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_packaged_folder_wins() {
    let exe = scratch("exe");
    let user = scratch("user2");
    fake_runtime(&exe.join("cuda"));
    fake_runtime(&user);
    let runtime = CudaRuntime::locate(Some(&exe), &user).unwrap();
    assert!(runtime.cuda_root.starts_with(exe.join("cuda")));
    std::fs::remove_dir_all(&exe).unwrap();
    std::fs::remove_dir_all(&user).unwrap();
}

#[test]
fn a_missing_library_is_named() {
    let dir = scratch("partial");
    fake_runtime(&dir);
    std::fs::remove_file(dir.join(CUDNN_FOLDER).join("lib").join("libcudnn.so.9")).unwrap();
    let missing = CudaRuntime::locate(None, &dir).unwrap_err();
    assert!(missing.to_string().contains("libcudnn.so.9"), "{missing}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_library_path_puts_the_runtime_first() {
    let runtime = CudaRuntime {
        cuda_root: "/r/cuda".into(),
        cudnn_root: "/r/cudnn".into(),
        onnxruntime_root: "/r/ort".into(),
        tensorrt_root: None,
    };
    assert_eq!(
        runtime.library_path(Some("/usr/lib")),
        "/r/cuda/lib:/r/cudnn/lib:/r/ort/lib:/usr/lib"
    );
    assert_eq!(
        runtime.library_path(Some("")),
        "/r/cuda/lib:/r/cudnn/lib:/r/ort/lib"
    );
    assert!(
        runtime
            .onnxruntime_library()
            .ends_with("lib/libonnxruntime.so")
    );
}

fn fake_tensorrt(base: &Path) {
    let trt = base.join(TENSORRT_FOLDER).join("lib");
    std::fs::create_dir_all(&trt).unwrap();
    for lib in REQUIRED_TENSORRT_LIBS {
        std::fs::write(trt.join(lib), b"").unwrap();
    }
    std::fs::write(trt.join("libnvinfer_plugin.so.10"), b"").unwrap();
    std::fs::write(trt.join("libnvinfer_builder_resource.so.10.14.1"), b"").unwrap();
    let ort = base.join(ONNX_RUNTIME_FOLDER).join("lib");
    std::fs::write(ort.join(TENSORRT_PROVIDER_LIB), b"").unwrap();
}

#[test]
fn tensorrt_is_optional() {
    let dir = scratch("no-tensorrt");
    fake_runtime(&dir);
    let runtime = CudaRuntime::locate(None, &dir).unwrap();
    assert!(!runtime.tensorrt_available());
    assert_eq!(runtime.tensorrt_version(), None);
    assert_eq!(runtime.lib_dirs().len(), 3);
    let missing = runtime
        .first_missing_tensorrt(&dir.join(TENSORRT_FOLDER))
        .unwrap();
    assert!(missing.ends_with(TENSORRT_PROVIDER_LIB), "{missing}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_complete_tensorrt_is_found_and_put_on_the_library_path() {
    let dir = scratch("tensorrt");
    fake_runtime(&dir);
    fake_tensorrt(&dir);
    let runtime = CudaRuntime::locate(None, &dir).unwrap();
    assert_eq!(runtime.tensorrt_root, Some(dir.join(TENSORRT_FOLDER)));
    assert_eq!(runtime.tensorrt_version(), Some(TENSORRT_VERSION));
    let path = runtime.library_path(Some("/usr/lib"));
    let parts: Vec<&str> = path.split(':').collect();
    assert_eq!(parts.len(), 5, "{path}");
    assert_eq!(
        parts[3],
        dir.join(TENSORRT_FOLDER).join("lib").display().to_string()
    );
    assert_eq!(parts[4], "/usr/lib");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn tensorrt_without_a_library_it_loads_is_not_used() {
    let dir = scratch("tensorrt-partial");
    fake_runtime(&dir);
    fake_tensorrt(&dir);
    let lib = dir.join(TENSORRT_FOLDER).join("lib");
    std::fs::remove_file(lib.join("libnvinfer_builder_resource.so.10.14.1")).unwrap();
    let runtime = CudaRuntime::locate(None, &dir).unwrap();
    assert!(!runtime.tensorrt_available());
    let missing = runtime
        .first_missing_tensorrt(&dir.join(TENSORRT_FOLDER))
        .unwrap();
    assert!(missing.contains("libnvinfer_builder_resource"), "{missing}");
    std::fs::write(lib.join("libnvinfer_builder_resource.so.10.14.1"), b"").unwrap();
    std::fs::remove_file(lib.join("libnvonnxparser.so.10")).unwrap();
    let runtime = CudaRuntime::locate(None, &dir).unwrap();
    assert!(!runtime.tensorrt_available());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_worker_env_names_onnx_runtime_and_the_tensorrt_folder() {
    let runtime = CudaRuntime {
        cuda_root: "/r/cuda".into(),
        cudnn_root: "/r/cudnn".into(),
        onnxruntime_root: "/r/ort".into(),
        tensorrt_root: Some("/r/trt".into()),
    };
    let env = runtime.worker_env();
    let library_path = &env.iter().find(|(k, _)| k == "LD_LIBRARY_PATH").unwrap().1;
    assert!(
        library_path.starts_with("/r/cuda/lib:/r/cudnn/lib:/r/ort/lib:/r/trt/lib"),
        "{library_path}"
    );
    let dylib = &env.iter().find(|(k, _)| k == "ORT_DYLIB_PATH").unwrap().1;
    assert_eq!(dylib, "/r/ort/lib/libonnxruntime.so");
}
