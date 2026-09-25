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
- Models (planned default): `~/.local/share/tbd-subtitles/models/`. Work folders (planned
  default): `~/.local/share/tbd-subtitles/work/`, configurable to a larger disk.

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
   distrobox-host-exec ffprobe -v error -show_entries stream=codec_type,codec_name,r_frame_rate,start_time -of compact "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 08.mp4"
   ```

   **Expected:** an `h264` video stream at `24/1` and an `aac` audio stream, both `start_time=0.000000`.

## CUDA libraries for ONNX Runtime

The `ort` crate's prebuilt GPU build needs CUDA 13 runtime libraries (cudart, cuBLAS, cuDNN ≥ 9.23).
Bazzite's image does not ship them and the root filesystem is read-only, so they are placed beside
the app binary (the `preload-dylibs` feature) or in a user folder the app points at. The working
recipe is written here during the M0.5 spike.

## Troubleshooting

- **`CUDA driver version is insufficient` or no GPU found inside the container:** the command ran
  in the container; prefix it with `distrobox-host-exec`.
- **`~/Projects/...` missing inside another distrobox:** most containers do not mount
  `/run/media`; `claude-desktop` does. Recreate a container with
  `--volume /run/media/system/Disk_2:/run/media/system/Disk_2:rslave` if needed.
- **Disk label changed:** the `/run/media/system/<label>` paths follow the disk label; update this
  runbook and CLAUDE.md.

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#hardware-and-host-rules) — how the app uses the GPU.
- [Rust ML stack](/documentation/research/rust_ml_stack.md#11-onnx-runtime-from-rust) — the `ort` CUDA details.
