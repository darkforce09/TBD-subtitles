# Model store

The models folder and the runtime folder: the compiled-in manifest of pinned model files and
runtime archives (CUDA 13, cuDNN, ONNX Runtime, TensorRT), the resumable downloads that check each
file's SHA-256 before using it, the unpacking of NVIDIA's `.tar.xz` and the `.tgz` and `.tar.gz`
archives, and the streamed install that keeps only some libraries of TensorRT's 7 GB tarball.

## Contents

```text
crates/inference/src/model_store/
├── archive.rs            runtime archives: download, xz or gzip decode on a thread, tar unpack minus the top folder
├── archive_libraries.rs  selected libraries of a streamed `.tar.gz`, hashed while read, installed only if it matched
├── download.rs           one pinned file: resumable https download, size and SHA-256 check, rename into place
├── manifest.rs           the pin types, every model file, and the runtime archives re-exported from `manifest/`
├── manifest/             the GPU runtime archives: CUDA, cuDNN, ONNX Runtime and TensorRT, with their folders
├── mod.rs                the folders, the error type, and the per-model fetch and completeness check
└── tests/                unit tests for the pins, path stripping, hashing, gzip unpacking, selected installs
```

## How it works

```text
fetch_model(models, id) ──▶ manifest::files_of(id) ──▶ download::fetch_verified ──▶ models/<id>/<file>
install_archive(archive, runtime) ──▶ fetch_verified ──▶ runtime/.archives/<file>.tar.xz
        └──▶ archive::unpack (lzma-rs or flate2 thread ─pipe─▶ tar) ──▶ runtime/<unpack_to>/
             └──▶ runtime/.fetched/<id> holds the archive's hash; the archive is deleted
install_libraries(TENSORRT_LIBRARIES, runtime) ──▶ runtime/.archives/<file> if placed, else https
        └──▶ hash + count ─▶ gunzip ─▶ tar: selected `lib/` members ──▶ runtime/.staging/<id>/lib/
             └──▶ size and hash match: rename to runtime/tensorrt-10.14/, marker; else remove staging
```

- `mod.rs` places both folders under `$XDG_DATA_HOME/tbd-subtitles/` (default
  `~/.local/share/tbd-subtitles/`): `models/` and `runtime/`. `app_data_dir` is that folder
  itself, where the pipeline also keeps the job work folders, the sign library and the GPU lock,
  and `claude_cli` its shared call slots. `is_complete` checks sizes only;
  hashes are checked when a file is downloaded, and only a matching file is renamed into place.
- `download.rs` writes `<file>.part`, resumes it with an HTTP `Range` request, hashes as it
  writes, and deletes the `.part` on a size or hash mismatch. Its progress callback returns
  `ControlFlow`: a break stops the download with `StoreError::Cancelled` and keeps the `.part`.
  `open_stream` gives a body to read once, for an archive never stored.
- `archive.rs` refuses tar entries with `..` or an absolute path, and lets a later archive's file
  replace an earlier one of the same name (the LICENSE files), so several archives merge into one
  toolkit folder. `is_installed` (re-exported as `is_archive_installed`) reads the marker. A name
  ending in `.tgz` or `.tar.gz` is gzip; any other is xz.
- `archive_libraries.rs` installs a `PinnedLibraries`: it reads the archive once, from a copy
  placed by hand in `runtime/.archives/` (deleted once installed) or straight from its URL,
  hashing and counting every byte, and writes only the members the selection names, those
  directly in a `lib/` folder (`lib/` or `targets/<triple>/lib/`), flat into a staging folder,
  with symlinks and hard links flattened to the bare names beside them. Only when the stream
  ended at the pinned size and hash, and every prefix matched a member, does the staging folder
  become `runtime/<unpack_to>/`; otherwise it is removed. A stop from the progress callback is
  `StoreError::Cancelled` and leaves nothing; there is no resume, so a stopped TensorRT download
  starts again. The marker holds the hash and the selection, so a changed selection unpacks
  again (`are_libraries_installed`).
- `manifest.rs` holds the model pins and re-exports the runtime archives `manifest/gpu_runtime.rs`
  pins, so `manifest::CUDA_FOLDER` and the rest keep their paths; `manifest/README.md` names the
  sources of the hashes. `runtime_archives` lists the archives Settings and the stack
  spike download; TensorRT is not among them, only the AppImage builder installs it.

## Boundaries

- Depends on: `ureq` (https with rustls), `sha2`, `lzma-rs`, `flate2`, `tar`; `std` for the files.
- Used by:
  - `crates/inference/src/cuda_runtime/` (the folder names and the TensorRT version) and
    `crates/inference/src/llm/claude_cli/shared_slots.rs` (`app_data_dir`);
  - the app: its settings page downloads models and runtime archives
    (`apps/tbd_subtitles/src/settings/services/model_downloads.rs`), and its environment, fonts,
    queue history, single-instance socket and service menu use the folders;
  - `crates/pipeline/` (the work, models and runtime folders, the sign library, the missing-model
    check, and the model pins and file hashes in step fingerprints) and `crates/stages/src/onscreen_text/`
    (`MODEL_FILES`);
  - `tools/stack_spike/` (the `fetch` command and the model folders), `tools/stack_spike_ggml/`,
    `tools/stack_spike_llm/`, `tools/appimage_builder/` (the runtime archives, TensorRT's
    libraries, the pinned FFmpeg archive and the bundled font) and `tools/visual_validation/`.
- Rules:
  - a file at its final path matches its pinned size and hash (`fetch_verified`; the
    `a_file_already_in_place_is_hashed_and_kept` test);
  - every pin is https with a 64-digit SHA-256 (`every_pin_is_https_with_a_sha256`,
    `tensorrt_is_nvidias_10_14_tarball_for_cuda_13`);
  - no archive entry lands outside its folder
    (`strip_first_drops_the_top_folder_and_refuses_escapes`,
    `library_name_takes_files_directly_in_a_lib_folder`);
  - nothing from a streamed archive is installed unless the whole stream matched its pin
    (`a_wrong_hash_installs_nothing_and_removes_what_was_unpacked`,
    `a_short_stream_is_a_size_mismatch`);
  - the store downloads models already exported and converts nothing; a conversion or a compiled
    engine happens in Rust or inside the runtime, where a measurement shows it pays (the crate
    header in `crates/inference/src/lib.rs`).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#models) — the models folder and
  its manifest.
- [Development environment](/documentation/runbooks/development_environment.md#cuda-libraries-for-onnx-runtime)
  — fetching and using the CUDA runtime.
- [Building the AppImage](/documentation/runbooks/building_the_appimage.md) — the TensorRT download
  `cargo appimage` makes and placing the tarball by hand.
