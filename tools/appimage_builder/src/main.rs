//! The AppImage builder: `cargo appimage [--skip-build] [--out <dir>]`.
//!
//! **Role:** builds TBD-subtitles as one self-contained AppImage: the three release binaries, the
//! CUDA, cuDNN and ONNX Runtime libraries the GPU workers load, each worker's own libraries,
//! a static FFmpeg, the Noto Sans JP font with its license, the desktop entry and icon, packed as
//! a zstd squashfs image behind the pinned AppImage runtime, in `dist/`.
//!
//! **Position:** a repository tool, run by a developer; `gpu_runtime`, `build`, `ffmpeg` and
//! `runtime` gather the parts, `app_dir` lays them out, `elf` walks and fixes up the libraries,
//! and `squashfs` packs the image. Depends on `app_icon`, `child_process` and `inference`.
//!
//! **Signals and state:** runs `cargo`, `git`, the built app and the bundled FFmpeg; downloads
//! into the runtime and models folders and `target/appimage/cache/`; lays out `target/appimage/AppDir/`;
//! prints each step and its time to stderr.
//!
//! **Invariants:** every step fails closed: a missing library, a wrong hash, an FFmpeg without
//! pulse or a worker without its GPU backend stops the build before an AppImage is written.

mod app_dir;
mod build;
mod elf;
mod ffmpeg;
mod gpu_runtime;
mod runtime;
mod squashfs;

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use child_process::Run;
use clap::Parser;

use app_dir::{APP_NAME, AppDir};
use build::Binaries;
use elf::Search;

/// Builds TBD-subtitles as one self-contained AppImage.
#[derive(Parser)]
#[command(name = "cargo appimage", version, about)]
struct Cli {
    /// Pack the binaries already in target/release instead of building them.
    #[arg(long)]
    skip_build: bool,
    /// The folder the AppImage is written to, relative to the repository root.
    #[arg(long, default_value = "dist")]
    out: PathBuf,
}

/// The name of the ggml worker binary, which the job runner looks for beside the app.
const GGML_WORKER: &str = "tbd-subtitles-ggml";
/// The local language-model worker, isolated from the app's ONNX Runtime.
const LOCAL_LLM_WORKER: &str = "tbd-subtitles-llm";

/// What the copied native libraries and the workers look for at run time.
const LIBRARY_RUNPATH: &str = "$ORIGIN";
const WORKER_RUNPATH: &str = "$ORIGIN/../lib";

fn main() -> ExitCode {
    let cli = Cli::parse();
    let started = Instant::now();
    match run(&cli) {
        Ok(image) => {
            eprintln!(
                "cargo appimage: wrote {} in {}",
                image.display(),
                minutes(started.elapsed())
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("cargo appimage: {error:#}");
            ExitCode::FAILURE
        }
    }
}

/// Every step in order; returns the stable-name AppImage.
fn run(cli: &Cli) -> Result<PathBuf> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .context("finding the repository root")?;
    let work = repo_root.join("target").join("appimage");
    let cache = work.join("cache");
    let out_dir = repo_root.join(&cli.out);

    let runtime_dir = step(
        "CUDA, cuDNN and ONNX Runtime archives",
        gpu_runtime::ensure_unpacked,
    )?;
    let binaries = step("release build", || {
        build::build(&repo_root, &runtime_dir, cli.skip_build)
    })?;
    let ffmpeg_bin = step("static FFmpeg", || ffmpeg::fetch(&cache))?;
    let head = step("AppImage runtime", || runtime::fetch(&cache))?;
    let app_dir = step("AppDir", || {
        lay_out(&work.join("AppDir"), &binaries, &runtime_dir, &ffmpeg_bin)
    })?;
    let version = step("smoke check", || smoke_check(&app_dir))?;
    let commit = runtime::short_commit(&repo_root)?;
    let image = out_dir.join(runtime::versioned_name(&version, &commit));
    step("squashfs image", || pack(app_dir.root(), &head, &image))?;
    let stable = runtime::finish(&image)?;
    let size = fs::metadata(&image).map(|m| m.len()).unwrap_or(0);
    eprintln!("  {} ({} MiB)", image.display(), size / (1 << 20));
    Ok(stable)
}

/// Run one step, printing its name and time; a failure names the step.
fn step<T>(label: &str, body: impl FnOnce() -> Result<T>) -> Result<T> {
    eprintln!("==> {label}");
    let started = Instant::now();
    let result = body().with_context(|| format!("{label} failed"))?;
    eprintln!("    {label}: {}", minutes(started.elapsed()));
    Ok(result)
}

/// `1m 05s` or `4.2s`.
fn minutes(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    if secs >= 60 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{:.1}s", elapsed.as_secs_f64())
    }
}

/// Lay out the whole AppDir at `root`.
fn lay_out(
    root: &Path,
    binaries: &Binaries,
    runtime_dir: &Path,
    ffmpeg_bin: &Path,
) -> Result<AppDir> {
    let app_dir = AppDir::create(root)?;
    let app = app_dir.add_binary(&binaries.app, APP_NAME)?;
    let worker = app_dir.add_binary(&binaries.ggml_worker, GGML_WORKER)?;
    let local_worker = app_dir.add_binary(&binaries.local_llm_worker, LOCAL_LLM_WORKER)?;
    let nothing = BTreeSet::new();
    let host_only = Search {
        dirs: Vec::new(),
        provided: &nothing,
    };
    elf::closure(&[app], &host_only).context("the app binary needs a library the host lacks")?;
    let bundled = gpu_runtime::bundle(runtime_dir, &app_dir.usr_bin())?;
    bundle_worker_libraries(&binaries.ggml_worker, &worker, &app_dir.usr_lib(), &bundled)?;
    bundle_worker_libraries(
        &binaries.local_llm_worker,
        &local_worker,
        &app_dir.usr_lib(),
        &bundled,
    )?;
    ffmpeg::bundle(ffmpeg_bin, &app_dir.usr_bin())?;
    bundle_font(app_dir.root())?;
    app_dir.finish()?;
    Ok(app_dir)
}

/// Bundle the verified Japanese review font and its redistribution license.
fn bundle_font(root: &Path) -> Result<()> {
    let models = inference::model_store::models_dir()?;
    let source = inference::model_store::fetch_model(&models, "visual-font", &mut |_, _, _| {
        std::ops::ControlFlow::Continue(())
    })?;
    let fonts = root.join("usr/share/fonts");
    let licenses = root.join("usr/share/licenses/tbd-subtitles");
    fs::create_dir_all(&fonts)?;
    fs::create_dir_all(&licenses)?;
    fs::copy(source.join("NotoSansJP.ttf"), fonts.join("NotoSansJP.ttf"))?;
    fs::copy(source.join("OFL.txt"), licenses.join("NotoSansJP-OFL.txt"))?;
    Ok(())
}

/// Copy a worker's native libraries, found through its build RUNPATH, into `usr_lib`, and point
/// every RUNPATH at the AppDir. Shared CUDA libraries are already in the runtime bundle.
fn bundle_worker_libraries(
    built: &Path,
    copy: &Path,
    usr_lib: &Path,
    provided: &BTreeSet<String>,
) -> Result<()> {
    let search = Search {
        dirs: Vec::new(),
        provided,
    };
    let found = elf::closure(&[built.to_path_buf()], &search)?;
    let bundled: BTreeSet<&str> = found.iter().skip(1).map(|f| f.name.as_str()).collect();
    for library in found.iter().skip(1) {
        let real = elf::copy_with_links(&library.dir, &library.name, usr_lib)?;
        let dynamic = elf::read_dynamic(&real)?;
        if dynamic.runpath.is_some() {
            elf::rewrite_runpath(&real, LIBRARY_RUNPATH)?;
        } else if let Some(needed) = dynamic.needed.iter().find(|n| bundled.contains(n.as_str())) {
            bail!(
                "{} needs {needed} but has no RUNPATH to find it beside itself",
                library.name
            );
        }
    }
    elf::rewrite_runpath(copy, WORKER_RUNPATH)
}

/// Run the copied app and FFmpeg; returns the app's version.
fn smoke_check(app_dir: &AppDir) -> Result<String> {
    let app = app_dir.usr_bin().join(APP_NAME);
    let output = Run::new(&app)
        .arg("--version")
        .timeout(Duration::from_secs(30))
        .output()
        .map_err(|e| anyhow!("{e}"))?;
    if output.code != 0 {
        bail!("{} --version exited with {}", app.display(), output.code);
    }
    let version = runtime::parse_app_version(&output.stdout)
        .ok_or_else(|| anyhow!("unreadable version line: {}", output.stdout.trim()))?
        .to_string();
    eprintln!("  tbd-subtitles {version}");
    let ffmpeg = ffmpeg::verify(&app_dir.usr_bin().join("ffmpeg"))?;
    eprintln!("  {ffmpeg}");
    Ok(version)
}

/// Write the runtime, then the squashfs image of `app_dir` behind it, to `image`.
fn pack(app_dir: &Path, head: &[u8], image: &Path) -> Result<()> {
    if let Some(parent) = image.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let _ = fs::remove_file(image);
    let mut out = File::create(image).with_context(|| format!("creating {}", image.display()))?;
    out.write_all(head)
        .with_context(|| format!("writing the runtime to {}", image.display()))?;
    squashfs::write_image(app_dir, &mut out, head.len() as u64)?;
    out.sync_all()
        .with_context(|| format!("flushing {}", image.display()))
}
