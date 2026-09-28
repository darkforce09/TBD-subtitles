**Status:** live

# Development environment

The machine this project is built and run on, the split between the host and the Claude Code
container, and the checks that show everything needed is present. Facts verified on the owner's
PC; re-run the checks after system updates.

## Prerequisites

- **Machine:** Bazzite (immutable Fedora, KDE), NVIDIA RTX 3070 8 GB (driver 615, CUDA 13 capable;
  about 5.5 GB free with the desktop running), Intel i7-14700K (28 threads), 31 GB RAM.
- **Rust:** rustup in `~/.cargo`, toolchain 1.95.0 installed (pinned by `rust-toolchain.toml`).
  Never install Rust through dnf or rpm-ostree.
- **FFmpeg:** 8.1 on the host (`/usr/bin/ffmpeg`, `/usr/bin/ffprobe`).
- **Git identity:** `sam <samdool01@gmail.com>`.

## Host and container

The Claude Code GUI (Claude Desktop) runs inside the distrobox container `claude-desktop`
(Debian 12):

| | Host | `claude-desktop` container |
|---|---|---|
| Sees `/run/media/system/…` (repo, videos) | yes | yes |
| Shared home: cargo, rustup, `~/.local/bin/claude` | yes | yes |
| NVIDIA driver library (`libcuda`) | yes | **no** |
| FFmpeg | 8.1 | 5.1 (too old; use the host's) |

Rule: build and test anywhere; run anything that touches the GPU, and FFmpeg, on the host through
`distrobox-host-exec` when working inside the container. The finished app is launched on the host.

## Paths

- Repository: `/run/media/system/Disk_2/Projects/TBD-subtitles` (`~/Projects` links to
  `/run/media/system/Disk_2/Projects`; 124 GB free on that disk).
- Test videos: `/run/media/system/Main_storage/Media/one_pace/` (3.3 TB free). Read-only for the
  project; see that folder's README.md.
- Models: `~/.local/share/tbd-subtitles/models/`. Job work directories:
  `~/.local/share/tbd-subtitles/work/<job id>/`, or under `--work-root` on a larger disk.

## Steps

1. Check the toolchain.

   ```bash
   cargo --version
   ```

   **Expected:** `cargo 1.95.0` when run inside the repository.

2. Check the GPU from wherever you are.

   ```bash
   distrobox-host-exec nvidia-smi --query-gpu=name,memory.total,memory.used --format=csv,noheader
   ```

   **Expected:** `NVIDIA GeForce RTX 3070, 8192 MiB, <used> MiB`. On the host itself, drop the
   `distrobox-host-exec` prefix.

3. Check FFmpeg on the host.

   ```bash
   distrobox-host-exec ffmpeg -hide_banner -version
   ```

   **Expected:** first line starts `ffmpeg version 8.1`.

4. Probe a test video.

   ```bash
   distrobox-host-exec ffprobe -v error -show_entries stream=codec_type,codec_name,r_frame_rate,start_time -of compact "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 11.mp4"
   ```

   **Expected:** an `h264` video stream at `24/1` and an `aac` audio stream, both `start_time=0.000000`.

5. Build the workspace and run its checks, from the repository root.

   ```bash
   cargo clippy --workspace --all-targets -- -D warnings
   ```

   **Expected:** `Finished` with no warning. Then `cargo fmt --all --check` prints nothing and
   `cargo test --workspace` ends every crate with `test result: ok`.

6. Run the repository gates.

   ```bash
   cargo gates
   ```

   **Expected:** every gate ends with `OK — N check(s), all held` and the command exits 0. Add
   `--with-untracked` to include new files before they are staged, and name one gate
   (`cargo gates link-check`) to run it alone.

7. Open the window on the host (the container has no display driver for it).

   ```bash
   distrobox-host-exec /run/media/system/Disk_2/Projects/TBD-subtitles/target/debug/tbd-subtitles gui
   ```

   **Expected:** a window titled "TBD Subtitles" with the queue on the left (empty the first
   time, else the queue the last window kept). A video named after `gui` joins the queue, with
   the log line `videos queued added=1` on stderr. Close the window to end the command.

8. Download the models and the GPU runtime (about 16 GiB the first time; later runs check what is
   there and download nothing).

   ```bash
   cargo run --release -p stack_spike -- fetch
   ```

   **Expected:** one line per model and runtime archive, then `models in …/models; runtime in
   …/runtime` and `checked <N> MiB against their pinned SHA-256`.

9. Build the ggml worker with CrispASR, under the CUDA 13.4 toolkit (about 3 minutes the first
   time).

   ```bash
   env PATH="$HOME/.local/share/tbd-subtitles/runtime/cuda-13.4/bin:$PATH" CUDACXX="$HOME/.local/share/tbd-subtitles/runtime/cuda-13.4/bin/nvcc" CUDAToolkit_ROOT="$HOME/.local/share/tbd-subtitles/runtime/cuda-13.4" CUDAARCHS=86 cargo build --release -p stack_spike_ggml --features crispasr
   ```

   **Expected:** `Finished release profile`; `ldd target/release/stack-spike-ggml` lists
   `libcrispasr.so.1` under `target/release/build/crispasr-sys-*/`.

10. Build the local language-model worker, under the CUDA 13.3 compiler (about 30
    minutes the first time).

    ```bash
    env PATH="$HOME/.local/share/tbd-subtitles/runtime/cuda-13.3-build/bin:$PATH" CUDA_ROOT="$HOME/.local/share/tbd-subtitles/runtime/cuda-13.3-build" CUDA_PATH="$HOME/.local/share/tbd-subtitles/runtime/cuda-13.3-build" CUDA_COMPUTE_CAP=86 LIBRARY_PATH="$HOME/.local/share/tbd-subtitles/runtime/cuda-13.4/lib" cargo build --release -p stack_spike_llm --features mistralrs
    ```

    **Expected:** `Finished release profile`. The kernels compile with nvcc 13.3 and link against
    the 13.4 cuBLAS, cuRAND and NVRTC (`LIBRARY_PATH`). Without `--features mistralrs` the build
    needs no toolkit and `stack-spike-llm` refuses its item.

11. Measure an item on the host, with nothing else using the GPU.

    ```bash
    distrobox-host-exec target/release/stack-spike run decode --video "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 11.mp4"
    ```

    **Expected:** `decode: Ok in <s> s (<x>× realtime)`; `stack-spike report --video …` prints the
    table. A GPU item with less than 5632 MiB of VRAM free is recorded as not run.

12. Build the app's two binaries: `tbd-subtitles`, then its Whisper worker `tbd-subtitles-ggml`
    under the CUDA 13.4 toolkit (about 3 minutes the first time). Both land in `target/release/`,
    where the runner finds the worker beside the app.

    ```bash
    cargo build --release -p tbd_subtitles
    ```

    ```bash
    env PATH="$HOME/.local/share/tbd-subtitles/runtime/cuda-13.4/bin:$PATH" CUDACXX="$HOME/.local/share/tbd-subtitles/runtime/cuda-13.4/bin/nvcc" CUDAToolkit_ROOT="$HOME/.local/share/tbd-subtitles/runtime/cuda-13.4" CUDAARCHS=86 cargo build --release -p tbd_subtitles_ggml --features crispasr
    ```

    **Expected:** `Finished release profile` twice; `ldd target/release/tbd-subtitles-ggml` lists
    `libcrispasr.so.1`. Without `--features crispasr` the worker builds but refuses every step.

13. Generate subtitles for a video on the host, with nothing else using the GPU.

    ```bash
    distrobox-host-exec target/release/tbd-subtitles process "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 11.mp4"
    ```

    **Expected:** one `> step` and `✓ step` line per step (`= step` for one still valid), then
    the paths of the subtitle file beside the video and of `report.md` in the job's work
    directory under `~/.local/share/tbd-subtitles/work/`. Running it again skips every step;
    `--rerun cues` redoes the cues, the quality check and the output only.

14. Run a job from the window on the host, with nothing else using the GPU.

    ```bash
    distrobox-host-exec target/release/tbd-subtitles gui
    ```

    Add a video with Add Videos… (or drop it on the window), press Start Queue, and select the job.

    **Expected:** each step's row turns from pending to running to done, with the time left under
    the job; when it ends, the report says whether it passes the quality check and lists the
    findings. Review lines opens the flagged lines; Play sounds the clip through the desktop's
    audio and shows its picture; Save and time again queues a "· 1 correction" run of the video
    that runs the review step, the cues, the quality check and the output only, and the old
    subtitle file moves to the work directory's `backup/`.

## CUDA libraries for ONNX Runtime

The GPU backends need the CUDA 13 runtime (cudart, cuBLAS, cuFFT, cuRAND, NVRTC, nvJitLink),
cuDNN 9 and ONNX Runtime 1.28. Bazzite's image ships none of them and its root filesystem is
read-only, so `stack-spike fetch` downloads NVIDIA's redistributable archives and Microsoft's ONNX
Runtime build, each pinned by size and SHA-256 in `crates/inference/src/model_store/manifest.rs`,
into the runtime folder `~/.local/share/tbd-subtitles/runtime/`:

| Folder | Holds | Needed for |
|---|---|---|
| `cuda-13.4/` | cudart, cuBLAS, cuFFT, cuRAND, NVRTC, nvJitLink, nvcc 13.4 and headers; `lib64` links to `lib` | every GPU worker at run time; building ggml (CrispASR) |
| `cudnn-9.26/` | cuDNN 9.26 for CUDA 13 | ONNX Runtime's CUDA provider at run time |
| `onnxruntime-1.28.2/` | `libonnxruntime.so` and its CUDA provider | the `ort` crate, which loads it at run time |
| `cuda-13.3-build/` | nvcc 13.3 with its headers | building mistral.rs only, whose build accepts toolkits up to 13.3 |

- **Why ONNX Runtime is loaded at run time:** the `ort` crate's prebuilt static library needs
  glibc 2.38 and GCC 13's libstdc++ to link, and the `claude-desktop` container has glibc 2.36 and
  GCC 12. With `load-dynamic`, nothing links at build time; Microsoft's build is loaded on the
  host.
- **At run time:** the process that starts a GPU worker sets `LD_LIBRARY_PATH` to the three `lib/`
  folders and `ORT_DYLIB_PATH` to `libonnxruntime.so` (`CudaRuntime::worker_env` in
  `crates/inference/src/cuda_runtime/mod.rs`). A packaged `<binary folder>/cuda/` with the same
  layout is looked in first.
- **ggml and ONNX Runtime:** loaded into one process they corrupt each other's heap, so the ggml
  models run in `stack-spike-ggml`, a binary of its own. That binary finds `libcrispasr` through
  the rpath its `build.rs` sets; `libggml-cuda` finds cudart through `LD_LIBRARY_PATH`.
- **Build toolkits:** CrispASR builds ggml with cmake and nvcc 13.4; mistral.rs builds its kernels
  with nvcc 13.3. Keep each build under its own toolkit: a crate built once under the other one
  keeps that toolkit's include path in its cached build output (`cargo clean -p candle-kernels`
  clears it). The container needs `cmake` and `libclang-dev` (`sudo apt install cmake
  libclang-dev`, done once).

## Troubleshooting

- **`CUDA driver version is insufficient` or no GPU found inside the container:** the command ran
  in the container; prefix it with `distrobox-host-exec`.
- **`~/Projects/...` missing inside another distrobox:** most containers do not mount
  `/run/media`; `claude-desktop` does. Recreate a container with
  `--volume /run/media/system/Disk_2:/run/media/system/Disk_2:rslave` if needed.
- **`cargo gates` exits 2 with "git not found":** the gates list the tracked files with `git`;
  run them where `git` is on the `PATH`.
- **The window does not open inside the container:** it has no display driver; launch the binary
  with `distrobox-host-exec` as in step 7.
- **`tbd-subtitles-ggml … is missing` at the `asr_whisper` step:** build the worker as in step 12,
  into the same folder as `tbd-subtitles`.
- **`process <pid> is already running it`:** another run holds the job's `job.lock`. Wait for it,
  or stop it; a lock whose process is gone is taken over.
- **A step fails:** the error names the step and quotes the end of its log,
  `logs/<step>.log` in the job's work directory. Fix the cause and run the same command again;
  the finished steps are skipped.
- **Disk label changed:** the `/run/media/system/<label>` paths follow the disk label; update this
  runbook and CLAUDE.md.

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#hardware-and-host-rules) — how the app uses the GPU.
- [Rust ML stack](/documentation/research/rust_ml_stack.md#11-onnx-runtime-from-rust) — the `ort` CUDA details.
