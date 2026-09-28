//! Building the app's two release binaries: `tbd-subtitles` and its Whisper worker.
//!
//! **Role:** run `cargo build --release -p tbd_subtitles`, then the ggml worker with the
//! `crispasr` feature under the CUDA 13.4 toolkit in the runtime folder, as the development
//! environment runbook does by hand, and check the worker really links CrispASR.
//!
//! **Position:** called by `main` after `gpu_runtime::ensure_unpacked`, which provides nvcc;
//! runs `cargo` through `child_process` and reads the worker with `elf`.
//!
//! **Signals and state:** starts `cargo` children in the repository root; reads `CARGO` (set by
//! `cargo run`) and `PATH`.
//!
//! **Invariants:** a worker built without `crispasr`, which would refuse every Whisper step, is
//! an error; the binaries returned exist.

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};
use child_process::Run;
use inference::model_store::manifest::CUDA_FOLDER;

use crate::elf;

/// The compute capability the ggml CUDA kernels are built for: the RTX 3070.
const CUDA_ARCHITECTURES: &str = "86";

/// The library the worker links when built with `crispasr`.
pub(crate) const CRISPASR_LIBRARY: &str = "libcrispasr.so.1";

/// The two binaries of a release build.
pub(crate) struct Binaries {
    /// `target/release/tbd-subtitles`.
    pub app: PathBuf,
    /// `target/release/tbd-subtitles-ggml`.
    pub ggml_worker: PathBuf,
}

/// Build both binaries (unless `skip`) and return where they are.
pub(crate) fn build(repo_root: &Path, runtime_dir: &Path, skip: bool) -> Result<Binaries> {
    let release = repo_root.join("target").join("release");
    let binaries = Binaries {
        app: release.join("tbd-subtitles"),
        ggml_worker: release.join("tbd-subtitles-ggml"),
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
    }
    for binary in [&binaries.app, &binaries.ggml_worker] {
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
    Ok(binaries)
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
