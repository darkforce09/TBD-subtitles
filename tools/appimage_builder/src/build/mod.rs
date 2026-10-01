//! Building the app and its separate Whisper and local translation workers.
//!
//! **Role:** run `cargo build --release -p tbd_subtitles`, then the ggml worker with the
//! `crispasr` feature under CUDA 13.4 and the local worker with `mistralrs` under the CUDA 13.3
//! compiler, linked to the CUDA 13.4 libraries. Check both workers' backend libraries.
//!
//! **Position:** called by `main` after `gpu_runtime::ensure_unpacked`, which provides nvcc;
//! runs `cargo` through `child_process` and reads both workers with `elf`.
//!
//! **Signals and state:** starts `cargo` children in the repository root; reads `CARGO` (set by
//! `cargo run`), `PATH` and `LIBRARY_PATH`.
//!
//! **Invariants:** workers built without their required GPU backends are errors; the binaries
//! returned exist, and the local worker has a RUNPATH the packaging step can rewrite.

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};
use child_process::Run;
use inference::model_store::manifest::{CUDA_BUILD_FOLDER, CUDA_FOLDER};

use crate::elf;

/// The compute capability the workers' CUDA kernels are built for: the RTX 3070.
const CUDA_ARCHITECTURES: &str = "86";

/// The library the worker links when built with `crispasr`.
pub(crate) const CRISPASR_LIBRARY: &str = "libcrispasr.so.1";

/// The three binaries of a release build.
pub(crate) struct Binaries {
    /// `target/release/tbd-subtitles`.
    pub app: PathBuf,
    /// `target/release/tbd-subtitles-ggml`.
    pub ggml_worker: PathBuf,
    /// `target/release/tbd-subtitles-llm`.
    pub local_llm_worker: PathBuf,
}

/// Build all binaries (unless `skip`) and return where they are.
pub(crate) fn build(repo_root: &Path, runtime_dir: &Path, skip: bool) -> Result<Binaries> {
    let release = repo_root.join("target").join("release");
    let binaries = Binaries {
        app: release.join("tbd-subtitles"),
        ggml_worker: release.join("tbd-subtitles-ggml"),
        local_llm_worker: release.join("tbd-subtitles-llm"),
    };
    if !skip {
        cargo(
            repo_root,
            &["build", "--release", "-p", "tbd_subtitles"],
            &[],
        )?;
        let cuda = runtime_dir.join(CUDA_FOLDER);
        let path = format!(
            "{}:{}",
            cuda.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let env = [
            ("PATH", path),
            (
                "CUDACXX",
                cuda.join("bin").join("nvcc").display().to_string(),
            ),
            ("CUDAToolkit_ROOT", cuda.display().to_string()),
            ("CUDAARCHS", CUDA_ARCHITECTURES.to_string()),
        ];
        let args = [
            "build",
            "--release",
            "-p",
            "tbd_subtitles_ggml",
            "--features",
            "crispasr",
        ];
        cargo(repo_root, &args, &env)?;
        build_local_worker(repo_root, runtime_dir)?;
    }
    for binary in [
        &binaries.app,
        &binaries.ggml_worker,
        &binaries.local_llm_worker,
    ] {
        if !binary.is_file() {
            bail!("{} is missing; run without --skip-build", binary.display());
        }
    }
    let worker = elf::read_dynamic(&binaries.ggml_worker)?;
    if !worker.needed.iter().any(|name| name == CRISPASR_LIBRARY) {
        bail!(
            "{} does not link {CRISPASR_LIBRARY}: it was built without the crispasr feature",
            binaries.ggml_worker.display()
        );
    }
    let worker = elf::read_dynamic(&binaries.local_llm_worker)?;
    for library in ["libcublas.so.13", "libcudart.so.13"] {
        if !worker.needed.iter().any(|name| name == library) {
            bail!(
                "{} does not link {library}: rebuild it with the mistralrs CUDA feature",
                binaries.local_llm_worker.display()
            );
        }
    }
    Ok(binaries)
}

/// Compile CUDA kernels with the supported compiler and reserve the worker's package RUNPATH.
fn build_local_worker(repo_root: &Path, runtime_dir: &Path) -> Result<()> {
    let compiler = runtime_dir.join(CUDA_BUILD_FOLDER);
    let libraries = runtime_dir.join(CUDA_FOLDER).join("lib");
    let env = [
        (
            "PATH",
            format!(
                "{}:{}",
                compiler.join("bin").display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        ),
        ("CUDA_ROOT", compiler.display().to_string()),
        ("CUDA_PATH", compiler.display().to_string()),
        ("CUDA_COMPUTE_CAP", CUDA_ARCHITECTURES.to_string()),
        (
            "LIBRARY_PATH",
            format!(
                "{}:{}",
                libraries.display(),
                std::env::var("LIBRARY_PATH").unwrap_or_default()
            ),
        ),
    ];
    let runpath = format!("link-arg=-Wl,-rpath,{}", libraries.display());
    cargo(
        repo_root,
        &[
            "rustc",
            "--release",
            "-p",
            "tbd_subtitles_llm",
            "--bin",
            "tbd-subtitles-llm",
            "--features",
            "mistralrs",
            "--",
            "-C",
            &runpath,
        ],
        &env,
    )
}

/// Run one cargo command in `repo_root`, failing with its stderr tail when it fails.
fn cargo(repo_root: &Path, args: &[&str], env: &[(&str, String)]) -> Result<()> {
    let program = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut run = Run::new(&program).args(args).cwd(repo_root);
    for (key, value) in env {
        run = run.env(*key, value.clone());
    }
    eprintln!("  cargo {}", args.join(" "));
    let output = run.output().map_err(|e| anyhow!("{e}"))?;
    if output.code != 0 {
        let tail: Vec<&str> = output.stderr.lines().rev().take(40).collect();
        let tail: Vec<&str> = tail.into_iter().rev().collect();
        bail!(
            "cargo {} exited with {}:\n{}",
            args.join(" "),
            output.code,
            tail.join("\n")
        );
    }
    Ok(())
}
