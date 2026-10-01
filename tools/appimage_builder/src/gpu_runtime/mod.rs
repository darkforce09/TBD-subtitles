//! The CUDA 13.4, cuDNN 9.26, ONNX Runtime 1.28.2 and TensorRT 10.14 libraries the AppImage
//! carries.
//!
//! **Role:** make sure the pinned runtime archives are unpacked in the user's runtime folder
//! (downloading what is missing, hash-checked; TensorRT's 7 GB tarball is streamed once and only
//! its libraries kept), then copy only the shared libraries the app's GPU workers load, plus
//! everything they NEED, into `usr/bin/cuda/<folder>/lib` in the AppDir, the place
//! `inference::cuda_runtime` looks first.
//!
//! **Position:** called by `main` before the build (the ggml build needs nvcc from the same
//! folder, and the local worker's build the CUDA 13.3 compiler unpacked beside it) and while
//! laying out the AppDir; uses `inference::model_store` and `elf`.
//!
//! **Signals and state:** reads and fills `~/.local/share/tbd-subtitles/runtime`; writes the
//! AppDir; prints download progress to stderr.
//!
//! **Invariants:** never copies nvcc, headers, static archives or the NVIDIA driver's libraries;
//! the copied set passes `CudaRuntime::first_missing` and has TensorRT available; every soname a
//! copied library needs is bundled or supplied by the host.

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use inference::cuda_runtime::{
    CudaRuntime, REQUIRED_CUDA_LIBS, REQUIRED_CUDNN_LIBS, REQUIRED_ONNX_RUNTIME_LIBS,
    REQUIRED_TENSORRT_LIBS, TENSORRT_LOADED_AT_RUN_TIME, TENSORRT_PROVIDER_LIB,
};
use inference::model_store::manifest::{
    CUDA_BUILD_ARCHIVES, CUDA_FOLDER, CUDNN_FOLDER, ONNX_RUNTIME_FOLDER, TENSORRT_FOLDER,
};
use inference::model_store::{self, CUDA_ARCHIVES, ONNX_RUNTIME_ARCHIVE, TENSORRT_LIBRARIES};

use crate::elf::{self, Search};

/// One runtime folder: its name under `runtime/`, the libraries the app loads from it, the name
/// prefixes of libraries loaded with `dlopen`, which no NEEDED entry names, and ONNX Runtime's
/// execution providers, which ONNX Runtime opens by file name.
struct Folder {
    name: &'static str,
    required: &'static [&'static str],
    loaded_at_run_time: &'static [&'static str],
    providers: &'static [&'static str],
}

/// The four folders, in the order the locator reads them. NVRTC opens its builtins library at
/// run time, cuFFT may open nvJitLink, cuDNN opens its engine libraries the same way, ONNX
/// Runtime opens its TensorRT provider, and that provider opens TensorRT's plugin library and the
/// builder opens its resource libraries.
const FOLDERS: [Folder; 4] = [
    Folder {
        name: CUDA_FOLDER,
        required: REQUIRED_CUDA_LIBS,
        loaded_at_run_time: &["libnvrtc-builtins.so.", "libnvJitLink.so."],
        providers: &[],
    },
    Folder {
        name: CUDNN_FOLDER,
        required: REQUIRED_CUDNN_LIBS,
        loaded_at_run_time: &["libcudnn"],
        providers: &[],
    },
    Folder {
        name: ONNX_RUNTIME_FOLDER,
        required: REQUIRED_ONNX_RUNTIME_LIBS,
        loaded_at_run_time: &[],
        providers: &[TENSORRT_PROVIDER_LIB],
    },
    Folder {
        name: TENSORRT_FOLDER,
        required: REQUIRED_TENSORRT_LIBS,
        loaded_at_run_time: TENSORRT_LOADED_AT_RUN_TIME,
        providers: &[],
    },
];

/// The user's runtime folder, with every pinned CUDA and ONNX Runtime archive unpacked in it and
/// TensorRT's libraries taken from its tarball.
pub(crate) fn ensure_unpacked() -> Result<PathBuf> {
    let runtime_dir = model_store::runtime_dir().map_err(|e| anyhow!("{e}"))?;
    let archives = CUDA_ARCHIVES
        .iter()
        .chain(CUDA_BUILD_ARCHIVES.iter())
        .chain(std::iter::once(&ONNX_RUNTIME_ARCHIVE));
    for archive in archives {
        if model_store::is_archive_installed(archive, &runtime_dir) {
            continue;
        }
        eprintln!("  downloading {}", archive.url);
        let mut progress = print_progress(archive.id);
        model_store::install_archive(archive, &runtime_dir, &mut progress)
            .map_err(|e| anyhow!("installing {}: {e}", archive.id))?;
        eprintln!();
    }
    if !model_store::are_libraries_installed(&TENSORRT_LIBRARIES, &runtime_dir) {
        let archive = &TENSORRT_LIBRARIES.archive;
        let placed = model_store::placed_archive_path(&TENSORRT_LIBRARIES, &runtime_dir);
        if placed.is_file() {
            eprintln!("  reading {}", placed.display());
        } else {
            eprintln!(
                "  streaming {} ({} MB, once; only the libraries are kept)",
                archive.url,
                archive.size / 1_000_000
            );
        }
        let mut progress = print_progress(archive.id);
        let kept = model_store::install_libraries(&TENSORRT_LIBRARIES, &runtime_dir, &mut progress)
            .map_err(|e| {
                anyhow!(
                    "installing TensorRT: {e}; if the download is refused, fetch the file in a \
                     browser and place it at {}",
                    placed.display()
                )
            })?;
        eprintln!();
        eprintln!("  kept {}", kept.join(", "));
    }
    Ok(runtime_dir)
}

/// A download progress callback that prints whole percentages on one line.
pub(crate) fn print_progress(label: &str) -> impl FnMut(u64, u64) -> ControlFlow<()> + '_ {
    let mut last = u64::MAX;
    move |held, total| {
        let percent = (held * 100).checked_div(total).unwrap_or(0);
        if percent != last {
            last = percent;
            eprint!("\r  {label}: {percent}%");
            let _ = std::io::stderr().flush();
        }
        ControlFlow::Continue(())
    }
}

/// Copy the runtime libraries from `runtime_dir` into `<usr_bin>/cuda/`; returns every library
/// name bundled, symlinks included, so other walks can leave them out.
pub(crate) fn bundle(runtime_dir: &Path, usr_bin: &Path) -> Result<BTreeSet<String>> {
    let lib_dir = |folder: &Folder| runtime_dir.join(folder.name).join("lib");
    let mut roots = Vec::new();
    for folder in &FOLDERS {
        let dir = lib_dir(folder);
        roots.extend(folder.required.iter().map(|name| dir.join(name)));
        roots.extend(folder.providers.iter().map(|name| dir.join(name)));
        roots.extend(run_time_loaded(&dir, folder.loaded_at_run_time)?);
    }
    let nothing = BTreeSet::new();
    let search = Search {
        dirs: FOLDERS.iter().map(lib_dir).collect(),
        provided: &nothing,
    };
    let found = elf::closure(&roots, &search)?;
    let bundle_root = usr_bin.join("cuda");
    let mut bundled = BTreeSet::new();
    for library in found {
        let folder = FOLDERS
            .iter()
            .find(|folder| lib_dir(folder) == library.dir)
            .ok_or_else(|| {
                anyhow!(
                    "{} was found in {}, outside the runtime folders",
                    library.name,
                    library.dir.display()
                )
            })?;
        let target = bundle_root.join(folder.name).join("lib");
        elf::copy_with_links(&library.dir, &library.name, &target)?;
        bundled.extend(names_in(&target)?);
    }
    let packaged = CudaRuntime::locate(Some(usr_bin), Path::new("/nonexistent"))
        .map_err(|e| anyhow!("the bundled runtime is incomplete: {e}"))?;
    if packaged.cuda_root != bundle_root.join(CUDA_FOLDER) {
        bail!("the runtime locator did not pick the bundled folder");
    }
    if !packaged.tensorrt_available() {
        let missing = packaged
            .first_missing_tensorrt(&bundle_root.join(TENSORRT_FOLDER))
            .unwrap_or_default();
        bail!("the bundled TensorRT is incomplete: {missing} is missing");
    }
    Ok(bundled)
}

/// The shared libraries in `dir` whose names start with one of `prefixes`: symlinks and real
/// files named `*.so.*`, never static archives.
fn run_time_loaded(dir: &Path, prefixes: &[&str]) -> Result<Vec<PathBuf>> {
    if prefixes.is_empty() {
        return Ok(Vec::new());
    }
    let mut paths: Vec<PathBuf> = names_in(dir)?
        .into_iter()
        .filter(|name| prefixes.iter().any(|p| name.starts_with(p)))
        .filter(|name| name.contains(".so.") && !name.ends_with(".a"))
        .map(|name| dir.join(name))
        .collect();
    paths.sort();
    Ok(paths)
}

/// The file names in `dir`.
fn names_in(dir: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("listing {}", dir.display()))? {
        let entry = entry.with_context(|| format!("listing {}", dir.display()))?;
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    Ok(names)
}

#[cfg(test)]
#[path = "tests/gpu_runtime.rs"]
mod tests;
