# CLAUDE.md — TBD-subtitles

Working context for AI sessions. Read this first, then [documentation/README.md](/documentation/README.md).
`AGENTS.md` is a symlink to this file; edit this one.

TBD-subtitles is a Rust desktop application that generates high-quality English subtitles for
local video files: every spoken line plus SDH sound cues, timed to the word, laid out to
Netflix's English rules. It runs locally on the owner's PC. A later milestone translates Japanese
text that appears on screen. The first job is the Muhn Pace Dressrosa English dub in
`/run/media/system/Main_storage/Media/one_pace/` (41 episodes; no dub subtitles exist anywhere).

**Current state:** milestones M0, M0.5, M1 and M2 are done. `tbd-subtitles process <video>` runs 18
resumable steps, with GPU steps in workers of `tbd-subtitles` and `tbd-subtitles-ggml`, and
writes the SRT, `report.md` and each step's time and memory. The owner accepted the Dressrosa 11
pilot ([pilot run](/documentation/research/pilot_dressrosa_11.md)), and a 128.9-minute video ran
in 19.2 minutes ([120-minute test](/documentation/research/long_video_120min.md)). M2 is the
desktop GUI: the window (`tbd-subtitles gui`) is redesigned to the owner's approved
macOS-like mockup and runs jobs itself: a toolbar and models banner, a sidebar of videos in Now,
Up Next and Done, the selected job's progress, its Overview report and Check Lines (clip playback,
corrections re-timed by correction runs), and Settings in a window of their own, saved as they
change, with model downloads, and a log window (Ctrl+L) that shows everything the app, its jobs
and the programs they start log, grouped by video and step, and every model call in full
([GUI](/documentation/features/gui.md)). Fix It has a stronger `claude` model (Opus by default,
beside the run's Sonnet) fix a finished job's flagged lines in three passes, each change kept or
undone by the owner, and the Overview shows its result when done; it runs on many videos at once
(Fix All, or after each job) under one cap on Claude calls
([Fix It](/documentation/features/fix_it.md)).
The owner runs it as a self-contained AppImage from Gear Lever, built by `cargo appimage`
(section 4). The owner ran the batch of Dressrosa 12–48 from the window and accepted it
(2026-09-29). M3, automation, is built and waits for the owner's test on the host: watch folders
that queue finished downloads while the app is open, Dolphin's "Generate subtitles" entry, one
window per session that later launches hand their videos to, a notification when a job ends, and
the window's icon ([automation](/documentation/features/automation.md),
[roadmap](/documentation/roadmap.md)).

## 1. Project laws

1. **No silent deferrals.** Do the whole ask. Only the owner defers work, in so many words
   ("defer X", "skip X", "not this pass"). Soft plan wording ("optional", "later") and
   agent-written deferral lists are not permission.
2. **Rust only.** Every tracked source file is Rust; the rest is Markdown, TOML or JSON. No
   Python, shell scripts, Makefiles or Node — not for tooling, not for model conversion, not once.
   A repository task is a Rust program (`cargo run -p <tool> -- …`), never a script.
3. **External programs:** the app runs FFmpeg and ffprobe only, as child processes (no custom
   decoder, no linking libav); the headless `claude` CLI is an optional language-model backend.
   Repository tooling under `tools/` may also run `git` and `cargo`.
4. **Inference runtimes:** pure-Rust engines (candle, burn, mistral.rs, earshot) come first. Rust
   crates that bind a native runtime (ONNX Runtime through `ort`, ggml through whisper-rs or
   transcribe-cpp) are used only where no pure-Rust option is competitive
   ([decisions](/documentation/decisions/)). Models are downloaded already exported (ONNX, GGUF,
   safetensors); we never convert models.
5. **Fast and bounded.** A 120-minute video processes end to end in 30 minutes or less on the
   RTX 3070, in bounded memory: audio is streamed and chunked, never held whole at 44.1 kHz.
6. **Resumable stages.** Each pipeline stage writes its output to the job's work directory and is
   skipped when a valid output already exists.
7. **One worker process per GPU stage.** GPU stages run as subcommands of the app binary in
   their own process: exiting frees VRAM and keeps native libraries apart.
8. **Never invent dialogue.** Every subtitle word comes from what a speech engine heard. The
   language model chooses between heard variants, fixes spelling and punctuation, and flags
   doubt; it never paraphrases.
9. **Clean boundaries.** GUI → core → stages → backends; lower layers never import higher ones.
   Names need zero context. Variants are grouped into subfolders, no flat dumps.
10. **File size and tests.** Production files stay under 500 lines, test files under 1000. Tests
    live in sibling files: `#[cfg(test)] #[path = "tests/<file>.rs"] mod tests;`.
11. **Comments** are present tense, with no history and no ticket numbers. A non-trivial module
    opens with a `//!` header: Role, Position, Signals and state, Invariants.
12. **Documentation** follows the [documentation standards](/documentation/standards/documentation_standards.md)
    and the [README standard](/documentation/standards/readme_standard.md): a README.md in every
    folder of `apps/`, `crates/`, `tools/` and `documentation/`, started from a
    [template](/documentation/standards/templates/README.md), and docs change in the same commit
    as the code.
13. **Git:** Conventional Commits, straight to `main`, explicit paths, `Co-Authored-By` trailer
    ([commit conventions](/documentation/standards/commit_conventions.md)).
14. **Source videos are read-only.** Subtitles go next to the video with the same base name.

## 2. Directory atlas

```text
TBD-subtitles/
├── .cargo/config.toml     the `cargo gates` alias
├── .editorconfig  .gitignore
├── AGENTS.md              symlink to this file
├── CLAUDE.md              this file
├── Cargo.toml  Cargo.lock the workspace: resolver 3, members only
├── README.md              what the project is, layout, documentation index
├── rust-toolchain.toml    Rust 1.95.0 for the whole workspace
├── apps/
│   ├── tbd_subtitles/     the binary: cli/, application/ (eframe shell), core/, and the feature
│   │                      folders job_queue/, job_report/, line_review/, settings/
│   └── tbd_subtitles_ggml/ the ggml worker binary: the Whisper steps (feature `crispasr`)
├── crates/                layers, lowest first:
│   ├── job_model/         0  stage names and the serde contracts between stages
│   ├── child_process/     0  external programs with deadlines, group kills, drained pipes
│   ├── media_io/          1  ffprobe, FFmpeg PCM streaming, shot changes
│   ├── subtitle_formats/  1  cue model, SRT/VTT/ASS writers, import
│   ├── inference/         1  onnx, ggml, candle, llm backends, model store, CUDA runtime
│   ├── stages/            2  one module folder per pipeline stage
│   └── pipeline/          3  step graph, resume, work directory, workers, tasks, runner, report
├── tools/
│   ├── appimage_builder/  `cargo appimage`: packages the app as a self-contained AppImage
│   ├── repo_gates/        `cargo gates`: every law a program can check
│   ├── stack_spike*/      the stack spike: measuring harness and its ggml and llm workers
│   └── verification_core/ fail-closed verdicts and reports for the gates
└── documentation/         goals, decisions, roadmap, architecture, research, features, runbooks,
                           standards and templates
```

Every folder under `apps/`, `crates/`, `tools/` and `documentation/` (except `tests/`) has a
README.md saying what it holds and where it stops; read it before changing the folder.

## 3. Environment

- **Host:** Bazzite (immutable Fedora), RTX 3070 8 GB (about 5.5 GB free with the desktop
  running), i7-14700K (28 threads), 31 GB RAM, FFmpeg 8.1, rustup with 1.95.0.
- **Claude Code GUI runs inside the distrobox `claude-desktop` (Debian 12).** It sees
  `/run/media/…` and the shared home, so cargo, rustup and `~/.local/bin/claude` work, but it has
  **no CUDA driver library**. Run anything that touches the GPU, and FFmpeg (the container's is
  5.1), on the host: `distrobox-host-exec <command>`. The finished app runs on the host.
- **Paths:** this repo is `/run/media/system/Disk_2/Projects/TBD-subtitles` (`~/Projects` is a
  symlink to it). Test videos: `/run/media/system/Main_storage/Media/one_pace/`.

Details and checks: [development environment](/documentation/runbooks/development_environment.md).

## 4. Commands

All run from the repository root; all four pass before every commit:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo gates                        # every law a program can check; exit 0, 1 or 2
```

More: `cargo gates <gate>` runs one gate (`cargo gates link-check --report`), `--path <dir>`
narrows it, `--with-untracked` includes new files. Open the window on the host:
`distrobox-host-exec target/debug/tbd-subtitles gui`. Build both app binaries (the ggml
worker under the CUDA 13.4 toolkit) and generate subtitles on the host as in steps 12 and 13 of
the [development environment](/documentation/runbooks/development_environment.md#steps) runbook:
`distrobox-host-exec target/release/tbd-subtitles process <video>`.

**Shipping the app (AppImage).** The owner runs the app as an AppImage from Gear Lever, not from
`target/`. After any change the owner should see in the app, rebuild it:

```bash
cargo appimage                     # in the container; about 2 minutes once the build is cached
```

It builds both binaries, bundles CUDA, cuDNN, ONNX Runtime and a pinned static FFmpeg, and writes
`dist/TBD-subtitles-x86_64.AppImage` (stable name) plus a copy named with the version and commit;
`--skip-build` repacks without building. Smoke-test on the host with
`distrobox-host-exec dist/TBD-subtitles-x86_64.AppImage --version`, then tell the owner to
re-import that file into Gear Lever, which keeps its own copy. If the FFmpeg download fails, the
BtbN autobuild is gone: re-pin it in `tools/appimage_builder/src/ffmpeg/`. Details: the
[AppImage runbook](/documentation/runbooks/building_the_appimage.md).

## 5. Where to look

| Question | Document |
|---|---|
| What are we building and why? | [Vision and goals](/documentation/vision_and_goals.md) |
| What is decided? | [Decisions](/documentation/decisions/) |
| What comes next? | [Roadmap](/documentation/roadmap.md) |
| How does the pipeline work? | [Pipeline](/documentation/architecture/pipeline.md) |
| Which Rust crates and models? | [Rust ML stack](/documentation/research/rust_ml_stack.md) |
| How should subtitles look? | [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md) |
| What does a word mean? | [Glossary](/documentation/glossary.md) |
