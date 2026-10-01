# AppImage builder source

The steps of `cargo appimage`, one module folder each, and the binary that runs them in order.

## Contents

```text
tools/appimage_builder/src/
├── app_dir/      the AppDir tree, the desktop entry, the drawn icon and the root symlinks
├── build/        the release builds of the app and of its ggml and local language-model workers
├── elf/          NEEDED, SONAME and RUNPATH reading, the library closure, copies and RUNPATH rewrite
├── ffmpeg/       the pinned static FFmpeg, its copy into the AppDir and the check it can play
├── gpu_runtime/  the pinned CUDA, cuDNN, ONNX Runtime and TensorRT archives and the libraries bundled from them
├── main.rs       the command line and the steps in order, each timed, failing closed
├── runtime/      the pinned AppImage type 2 runtime and the output names
└── squashfs/     the zstd squashfs image of the AppDir, written behind the runtime
```

## How it works

`main.rs` makes sure the runtime archives are unpacked and TensorRT's libraries installed
(`gpu_runtime`), builds all three binaries (`build`), fetches FFmpeg (`ffmpeg`) and the AppImage
runtime (`runtime`), then lays out the AppDir (`app_dir`): the binaries, the GPU libraries
(TensorRT and ONNX Runtime's TensorRT provider among them) with their NEEDED closure, the ggml worker's
and the local translation worker's own libraries with their RUNPATH pointed at the AppDir (`elf`),
FFmpeg, and a checksum-pinned Noto Sans JP font with its redistribution license. A smoke check runs the
copied app's `--version` and the copied FFmpeg's `-version`, `-filters` and `-devices`. Last,
`main.rs` writes the runtime's bytes to the output file and `squashfs` writes the image behind
them, at that offset, so the file is runtime and image in one.

## Public surface

None: every module is private to the binary.

## Boundaries

- Depends on: `app_icon`, `child_process`, `inference`, `anyhow`, `clap`, `object`, `backhand`,
  `png`.
- Used by: nothing outside the binary.
- Rules:
  - each step fails closed and names itself in the error (`step` in `main.rs`);
  - the smoke check runs before the image is written, so a broken AppDir never becomes an
    AppImage.
