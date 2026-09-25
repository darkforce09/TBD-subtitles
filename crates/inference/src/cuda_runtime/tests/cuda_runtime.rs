use std::path::Path;

use super::*;

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
    };
    assert_eq!(
        runtime.library_path(Some("/usr/lib")),
        "/r/cuda/lib:/r/cudnn/lib:/usr/lib"
    );
    assert_eq!(runtime.library_path(Some("")), "/r/cuda/lib:/r/cudnn/lib");
}
