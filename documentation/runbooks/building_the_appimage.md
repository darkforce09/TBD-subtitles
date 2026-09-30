**Status:** live

# Building the AppImage

Package `tbd-subtitles` as one self-contained `.AppImage`: the app's three binaries, a bundled CUDA
13.4 + cuDNN 9.26 + ONNX Runtime 1.28.2 runtime, and a static FFmpeg, so it runs on the host with
only the NVIDIA driver, X11/EGL, PulseAudio, the desktop portal and FUSE. Building takes about two
minutes with a cached `cargo build`, longer the first time it downloads the CUDA toolkit and the
runtime and FFmpeg archives.

## Prerequisites

- Run inside the `claude-desktop` container, which has `cmake` and `libclang-dev`
  (`sudo apt install cmake libclang-dev`, done once) — see [development
  environment](/documentation/runbooks/development_environment.md#steps) steps 1 and 12 for the
  toolchain and the CUDA 13.4 toolkit under `~/.local/share/tbd-subtitles/runtime/cuda-13.4`,
  which the builder downloads itself if missing. The local translation worker uses the pinned
  CUDA 13.3 compiler with CUDA 13.4 runtime libraries; the builder supplies that compiler too.
- Network access, to fetch the pinned CUDA toolkit, FFmpeg and AppImage runtime archives into
  `target/appimage/cache/` the first time.

## Steps

1. Build the AppImage, from the repository root.

   ```bash
   cargo appimage
   ```

   **Expected:** one line per step (build, gather the CUDA runtime, download FFmpeg, lay out
   `AppDir`, pack the squashfs image), then the path
   `dist/TBD-subtitles-<version>-<git short>-x86_64.AppImage` and its size (around 1.5 GB).
   `dist/TBD-subtitles-x86_64.AppImage` is a hard link to the same file, for a stable name.

2. Rebuild the image from an already-built `AppDir` without rebuilding the app, after only a docs
   or packaging change.

   ```bash
   cargo appimage --skip-build
   ```

   **Expected:** the same output, skipping the `cargo build` step; finishes in well under a
   minute.

3. Smoke-test the image on the host (the container has no NVIDIA driver library).

   ```bash
   distrobox-host-exec dist/TBD-subtitles-x86_64.AppImage --version
   ```

   **Expected:** `tbd-subtitles <version>`.

4. Re-import `dist/TBD-subtitles-x86_64.AppImage` into Gear Lever, then launch the app once from
   the application menu.

   The AppImage's desktop entry starts the app as `tbd-subtitles %F`, so videos opened with it
   reach the app as arguments, and its `MimeType` line (`video/mp4`, `video/x-matroska`,
   `video/webm`, `video/quicktime`, `video/x-msvideo`, `video/mp2t`, `video/x-m4v`) offers it
   under "Open With" in the file manager. Each start as an AppImage also writes the Dolphin
   service menu `~/.local/share/kio/servicemenus/tbd-subtitles.desktop` (under `$XDG_DATA_HOME`
   when that is set), executable, as KDE requires: one action, "Generate subtitles", on every
   video's right-click menu, which runs the AppImage named by `$APPIMAGE` as
   `process --enqueue` with the chosen videos. The AppImage's icon is copied to
   `~/.local/share/tbd-subtitles/tbd-subtitles.png` for the menu to show. Both are rewritten only
   when their contents differ. Gear Lever keeps its own copy of the AppImage, under a path of its
   own, so the first launch after a re-import points the menu at that copy.

   **Expected:** right-clicking a video in Dolphin shows "Generate subtitles" with the app's
   icon; `grep Exec= ~/.local/share/kio/servicemenus/tbd-subtitles.desktop` names the AppImage
   Gear Lever holds.

   To remove the service menu, delete that file (and the icon copy, if wanted); the next start
   of the AppImage writes it again.

   ```bash
   rm ~/.local/share/kio/servicemenus/tbd-subtitles.desktop
   ```

## Verify

```bash
distrobox-host-exec env -u LD_LIBRARY_PATH dist/TBD-subtitles-x86_64.AppImage process "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 11.mp4" --work-root /tmp/appimage-verify
```

**Expected:** all 29 steps run (the Whisper steps on the GPU, through the bundled `ggml` worker
and its bundled `libcrispasr.so.1`), ending with the subtitle file written beside a copy of the
video and `report.md` in `/tmp/appimage-verify`. Comparing it to the [Dressrosa 11
pilot](/documentation/research/pilot_dressrosa_11.md) subtitle file shows the same dialogue lines.
Visual translation writes a combined ASS and uses the bundled local-model worker; with Replace
text in the video on, the job also writes `<name>.localized.mkv` (encoded with `hevc_nvenc` by the
bundled FFmpeg when it reaches the driver, else libx264) and `<name>.localized.ass`; a `<name>.localized.mkv` another job wrote (under another
work root) stops that last step with a message to move it away. Settings →
On-screen Text lists the required model downloads. Check Text must render Japanese correctly
using the bundled Noto Sans JP font and preview the exported ASS through FFmpeg.

## Troubleshooting

- **`fuse: device not found` or the AppImage exits immediately:** FUSE is not available (common
  inside a container); run it with `--appimage-extract-and-run` instead of executing it directly.
- **The FFmpeg or AppImage runtime download 404s:** the pinned BtbN autobuild or `type2-runtime`
  release was deleted upstream; re-pin the URL and SHA-256 in `tools/appimage_builder` to a
  current build and rerun `cargo appimage`. Files already in `target/appimage/cache/` are reused
  as long as their SHA-256 still matches, so a local rebuild is unaffected.
- **`tbd-subtitles-ggml … built without crispasr` during the build step:** the ggml worker built
  without the CUDA 13.4 toolkit on `PATH`; `cargo appimage` sets `CUDACXX`, `CUDAToolkit_ROOT` and
  `CUDAARCHS=86` itself, so this means the toolkit under
  `~/.local/share/tbd-subtitles/runtime/cuda-13.4` is missing or incomplete — delete it and rerun
  so the builder downloads it again.
- **Settings → system check does not say "bundled" for FFmpeg or the runtime:** the AppImage was
  launched with its own binary run directly instead of through `AppRun`, or an old extracted
  `AppDir` is on `PATH` ahead of it; launch the `.AppImage` file itself.

- **"Generate subtitles" is missing from Dolphin, or starts an AppImage that is gone:** the app
  has not started as an AppImage since the last re-import, so the menu is missing or names the
  earlier file. Launch the app once from the application menu; the menu then names the current
  AppImage. Starting the binary from `target/` writes no menu.

- **Where is the app's log when launched from Gear Lever?** Gear Lever drops stderr; the
  window writes its log to `~/.local/state/tbd-subtitles/tbd-subtitles.log` too, emptied at each
  start. A button that asks the desktop (Add Videos…, Open in Player, Show in Folder) and fails
  logs a warning there.

## Related documentation

- [Development environment](/documentation/runbooks/development_environment.md) — building the
  app's own binaries and the CUDA toolkit the AppImage build step needs.
- [Decisions: stack and pipeline](/documentation/decisions/stack_and_pipeline.md#2026-09-28--ship-as-one-appimage-bundling-cuda-cudnn-onnx-runtime-and-ffmpeg) — why the image bundles CUDA, cuDNN, ONNX Runtime and FFmpeg.
- [Decisions: foundations](/documentation/decisions/foundations.md#2026-09-28--repository-tooling-may-also-run-ffmpeg-and-the-apps-own-binaries) — why `tools/appimage_builder` may run FFmpeg and the app's own binaries.
