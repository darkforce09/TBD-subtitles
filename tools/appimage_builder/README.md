# AppImage builder

The `appimage_builder` binary behind `cargo appimage`: it builds TBD-subtitles and packs it, with
the CUDA, cuDNN and ONNX Runtime libraries and a static FFmpeg, into one self-contained AppImage in
`dist/` that a desktop can start with nothing installed but the NVIDIA driver.

## Contents

```text
tools/appimage_builder/
├── Cargo.toml  the `appimage_builder` binary package and the reason for each dependency
└── src/        the steps: runtime libraries, build, FFmpeg, AppDir, ELF fix-ups, squashfs, output
```

## How it works

`cargo appimage` is an alias in `.cargo/config.toml` for
`cargo run --release --package appimage_builder --`. The binary runs its steps in order, prints
each with its time, and stops at the first failure, before an AppImage is written:

```text
runtime archives ─> release build ─> static FFmpeg ─> AppImage runtime ─> AppDir ─> smoke check ─> image
(nvcc, CUDA libs)   (app, ggml and   (pinned, hash)   (pinned, hash)      (layout,   (--version,   (squashfs
                     llm workers)                                         font)      -devices)     behind runtime)
```

No packaging program outside Rust takes part: the ELF walk and the RUNPATH rewrite use `object`,
the squashfs image is written by `backhand` with zstd, and the desktop entry and the icon are
generated, so no packaging file is tracked. The result is
`dist/TBD-subtitles-<version>-<commit>-x86_64.AppImage` and a copy named
`TBD-subtitles-x86_64.AppImage`. The AppDir inside it:

```text
AppRun -> usr/bin/tbd-subtitles    tbd-subtitles.desktop    tbd-subtitles.png    .DirIcon
usr/bin/tbd-subtitles
usr/bin/tbd-subtitles-ggml, usr/bin/tbd-subtitles-llm (RUNPATH $ORIGIN/../lib)
usr/bin/cuda/{cuda-13.4,cudnn-9.26,onnxruntime-1.28.2}/lib    the libraries the GPU workers load
usr/bin/ffmpeg/{ffmpeg,ffprobe}                                static FFmpeg 8.1 with pulse
usr/lib/libcrispasr.so.1, libggml*.so.0, …                     each worker's own libraries (RUNPATH $ORIGIN)
usr/share/fonts/NotoSansJP.ttf, usr/share/licenses/tbd-subtitles/NotoSansJP-OFL.txt
usr/share/applications, usr/share/icons/hicolor/256x256/apps
```

The app finds everything beside its own executable: the CUDA locator in `crates/inference` looks
in `<exe dir>/cuda/` first, and the job runner finds both workers beside the app.

## Getting started

Run these from the repository root:

```bash
cargo appimage                    # build, gather and pack; the first run downloads about 2.5 GB
cargo appimage --skip-build       # pack the binaries already in target/release
cargo test -p appimage_builder    # the unit tests, about a second
```

A finished run ends with `cargo appimage: wrote …/dist/TBD-subtitles-x86_64.AppImage in …`.

## Configuration

- `--skip-build`: pack `target/release/tbd-subtitles`, `tbd-subtitles-ggml` and
  `tbd-subtitles-llm` as they are.
- `--out <dir>`: the output folder, relative to the repository root; default `dist`.
- `XDG_DATA_HOME` or `HOME`: where the runtime and models folders lie, read through
  `inference::model_store::runtime_dir` and `models_dir`; the bundled font is fetched into the
  models folder.
- `CARGO` (set by `cargo run`) names the cargo that builds; `PATH` is extended with the CUDA
  13.4 `bin/` folder for the ggml build, and with the CUDA 13.3 compiler's `bin/` folder, plus
  `LIBRARY_PATH` with the CUDA 13.4 libraries, for the local language-model worker's build.

Downloads are cached in `target/appimage/cache/`; the AppDir is laid out in
`target/appimage/AppDir/`.

## Public surface

- The binary `appimage-builder`, run as `cargo appimage [--skip-build] [--out <dir>]`.
- No library: every module is private to the binary.

## Boundaries

- Depends on: `crates/app_icon` (the icon's pixels); `crates/child_process` (running `cargo`,
  `git`, the built app and the bundled FFmpeg); `crates/inference` (the runtime folder names, the
  required library lists, the pinned archives and their hash-checked download and unpack);
  `anyhow`, `clap`, `object`, `backhand` and `png`; the programs `cargo` and `git`.
- Used by: a developer packaging the app, through the `appimage` alias; no crate depends on it.
- Rules:
  - it depends on no workspace crate but `app_icon`, `child_process` and `inference` (the tool
    table in `tools/repo_gates/src/layout.rs`, `cargo gates crate-layering`);
  - every download is pinned by URL and SHA-256 and checked before use (`fetch_verified`,
    `install_archive`);
  - the NVIDIA driver's libraries and glibc are never bundled (`host_libraries_are_recognised_by_stem`).

## Related documentation

- [Development environment](/documentation/runbooks/development_environment.md) — the CUDA
  toolkit and the build commands this tool runs.
- [Repository tooling may run git and cargo](/documentation/decisions/foundations.md#2026-09-25--repository-tooling-may-run-git-and-cargo)
  — which programs the tools may start.
