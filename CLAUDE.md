# CLAUDE.md — TBD-subtitles

Working context for AI sessions. Read this first, then [documentation/README.md](/documentation/README.md).
`AGENTS.md` is a symlink to this file; edit this one.

TBD-subtitles is a Rust desktop application that generates high-quality English subtitles for
local video files: every spoken line plus SDH sound cues, timed to the word, laid out to
Netflix's English rules. It runs locally on the owner's PC. A later milestone translates Japanese
text that appears on screen. The first job is the Muhn Pace Dressrosa English dub in
`/run/media/system/Main_storage/Media/one_pace/` (41 episodes; no dub subtitles exist anywhere).

**Current state:** milestone M0 is done: the Cargo workspace, the `tbd-subtitles` binary (an
eframe window and the `gui`, `process` and `worker` subcommands), every crate and module folder
with its README, and the `cargo gates` checks. No pipeline stage is built yet. Next step:
milestone M0.5 in the [roadmap](/documentation/roadmap.md), after the owner reviews M0.

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
   ([decisions](/documentation/decisions.md)). Models are downloaded already exported (ONNX, GGUF,
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
│   └── tbd_subtitles/     the binary: cli/, application/ (eframe shell), core/, and the feature
│                          folders job_queue/, job_report/, line_review/, settings/
├── crates/                layers, lowest first:
│   ├── job_model/         0  stage names and the serde contracts between stages
│   ├── child_process/     0  external programs with deadlines, group kills, drained pipes
│   ├── media_io/          1  ffprobe, FFmpeg PCM streaming, shot changes
│   ├── subtitle_formats/  1  cue model, SRT/VTT/ASS writers, import
│   ├── inference/         1  onnx, ggml, candle, llm backends, model store
│   ├── stages/            2  one module folder per pipeline stage
│   └── pipeline/          3  stage graph, resume, worker processes, progress, work directory
├── tools/
│   ├── repo_gates/        `cargo gates`: every law a program can check
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
`distrobox-host-exec target/debug/tbd-subtitles gui`.

## 5. Where to look

| Question | Document |
|---|---|
| What are we building and why? | [Vision and goals](/documentation/vision_and_goals.md) |
| What is decided? | [Decisions](/documentation/decisions.md) |
| What comes next? | [Roadmap](/documentation/roadmap.md) |
| How does the pipeline work? | [Pipeline](/documentation/architecture/pipeline.md) |
| Which Rust crates and models? | [Rust ML stack](/documentation/research/rust_ml_stack.md) |
| How should subtitles look? | [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md) |
| What does a word mean? | [Glossary](/documentation/glossary.md) |
