**Status:** live

# Template: runbook

**When to use:** a procedure someone runs step by step: setting up or checking the environment,
running a batch of videos, recovering a stuck job, handing the project to the next session.
Runbooks live in `documentation/runbooks/`, one file each, named in snake_case after the
procedure; a runbook longer than 500 lines becomes a folder with a README index. The
[documentation standards](/documentation/standards/documentation_standards.md) fix the sections;
the [README standard](/documentation/standards/readme_standard.md) holds the writing rules a
runbook shares with READMEs.

## Skeleton

Copy the block and replace every `<…>` placeholder; each one says what goes there. Steps are
numbered; each holds one command in its own block and an **Expected:** line. A runbook may add a
section of reference facts, such as paths or a host and container table, between Prerequisites and
Steps.

````markdown
**Status:** live

# <The procedure, in plain words: what it achieves>

<One to three sentences: what the procedure does, when to run it, and how long it takes.>

## Prerequisites

- <a tool, access, file or running program the procedure needs, and how to tell it is there>

## Steps

1. <What the step does, and where to run it: the host, the container, or the repository root.>

   ```bash
   <one command>
   ```

   **Expected:** <the output or state that shows the step worked, quoted from a safe run or from
   the code that prints it>

## Verify

<The check that proves the whole procedure worked, as one command block and its **Expected:**
line.>

## Troubleshooting

- **<what the operator sees, quoted>:** <why it happens, as the code or the environment shows it;
  what to do>

## Related documentation

- [<document title>](/documentation/<path to the document>) — <what it covers>
````

Every command is checked before it is written: a `cargo gates` command against the gate runner
(`cargo gates link-check` fails a runbook that cites a gate that does not exist, fenced blocks
included), anything else against its own `--help`. An **Expected:** line comes from a safe run
(`--help`, `--version`, a probe, a read-only query) or, where no safe run exists, from the code
that prints the output; the writer never runs a destructive command to check a step and never
writes to a source video. A command that touches the GPU or FFmpeg says where it runs: from inside
the `claude-desktop` container it is prefixed with `distrobox-host-exec`.

## Worked sample

Written from `documentation/runbooks/development_environment.md` and checked by running each
command from inside the container. The sample keeps the toolchain, GPU, FFmpeg and probe checks,
and adds a Verify step. The sample sits in a fenced block, so no gate reads its links; the
runbook itself is written from the same facts and may differ.

````markdown
**Status:** live

# Development environment

The checks that show the machine can build the app and run its GPU and FFmpeg work, from the host
or from the Claude Code container. Run them after a system update; they take about a minute, and
the first build takes several.

## Prerequisites

- rustup in `~/.cargo`, with the toolchain that `rust-toolchain.toml` pins (1.95.0).
- The NVIDIA driver and FFmpeg 8.1 on the host. The `claude-desktop` container has no NVIDIA
  driver library and an older FFmpeg, so from inside it every GPU and FFmpeg command runs on the
  host through `distrobox-host-exec`.

## Steps

1. Check the toolchain, from the repository root.

   ```bash
   cargo --version
   ```

   **Expected:** `cargo 1.95.0` followed by its build hash and date.

2. Check the GPU and its free memory.

   ```bash
   distrobox-host-exec nvidia-smi --query-gpu=name,memory.total,memory.used --format=csv,noheader
   ```

   **Expected:** `NVIDIA GeForce RTX 3070, 8192 MiB, <used> MiB`. On the host itself, drop the
   `distrobox-host-exec` prefix.

3. Check FFmpeg on the host.

   ```bash
   distrobox-host-exec ffmpeg -hide_banner -version
   ```

   **Expected:** the first line starts `ffmpeg version 8.1`.

4. Probe the pilot episode.

   ```bash
   distrobox-host-exec ffprobe -v error -show_entries stream=codec_type,codec_name,r_frame_rate,start_time -of compact "/run/media/system/Main_storage/Media/one_pace/done/[Muhn Pace] Dressrosa 08.mp4"
   ```

   **Expected:** an `h264` video stream at `r_frame_rate=24/1` and an `aac` audio stream, both
   with `start_time=0.000000`.

## Verify

```bash
cargo run -q -p tbd_subtitles -- --help
```

**Expected:** the app builds and prints
`Generates English subtitles for local videos: GUI, command line and GPU workers`, then
`Usage: tbd-subtitles [COMMAND]` and the subcommands `gui`, `process` and `worker`.

## Troubleshooting

- **`nvidia-smi: command not found`, or no GPU found:** the command ran inside the container,
  which has no NVIDIA driver library; prefix it with `distrobox-host-exec`.
- **`ffmpeg version 5.1` on the first line:** the container's own FFmpeg answered; prefix the
  command with `distrobox-host-exec`.
- **`~/Projects/...` missing inside another distrobox:** most containers do not mount
  `/run/media`; `claude-desktop` does. Recreate the container with
  `--volume /run/media/system/Disk_2:/run/media/system/Disk_2:rslave`.

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#hardware-and-host-rules) — how
  the app uses the GPU and where it runs.
- [Rust ML stack](/documentation/research/rust_ml_stack.md#11-onnx-runtime-from-rust) — the CUDA
  libraries the ONNX Runtime crate needs.
````
