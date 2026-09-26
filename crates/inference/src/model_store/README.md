# Model store

The models folder and the runtime folder: the compiled-in manifest of pinned model files and
runtime archives (CUDA 13, cuDNN, ONNX Runtime), the resumable downloads that check each file's
SHA-256 before using it, and the unpacking of NVIDIA's `.tar.xz` and Microsoft's `.tgz` archives.

## Contents

```text
crates/inference/src/model_store/
├── archive.rs   runtime archives: download, xz or gzip decode on a thread, tar unpack minus the top folder
├── download.rs  one pinned file: resumable https download, size and SHA-256 check, rename into place
├── manifest.rs  every model file and runtime archive, pinned by URL, size and SHA-256
├── mod.rs       the folders, the error type, and the per-model fetch and completeness check
└── tests/       unit tests for the pins, path stripping, hashing and the folder lookups
```

## How it works

```text
fetch_model(models, id) ──▶ manifest::files_of(id) ──▶ download::fetch_verified ──▶ models/<id>/<file>
install_archive(archive, runtime) ──▶ fetch_verified ──▶ runtime/.archives/<file>.tar.xz
        └──▶ archive::unpack (lzma-rs or flate2 thread ─pipe─▶ tar) ──▶ runtime/<unpack_to>/
             └──▶ runtime/.fetched/<id> holds the archive's hash; the archive is deleted
```

- `mod.rs` places both folders under `$XDG_DATA_HOME/tbd-subtitles/` (default
  `~/.local/share/tbd-subtitles/`): `models/` and `runtime/`. `is_complete` checks sizes only;
  hashes are checked when a file is downloaded, and only a matching file is renamed into place.
- `download.rs` writes `<file>.part`, resumes it with an HTTP `Range` request, hashes as it
  writes, and deletes the `.part` on a size or hash mismatch. Its progress callback returns
  `ControlFlow`: a break stops the download with `StoreError::Cancelled` and keeps the `.part`.
- `archive.rs` refuses tar entries with `..` or an absolute path, and lets a later archive's file
  replace an earlier one of the same name (the LICENSE files), so several archives merge into one
  toolkit folder. `is_installed` (re-exported as `is_archive_installed`) reads the marker.
- `manifest.rs` holds the Hugging Face LFS hashes of the models, the hashes from NVIDIA's
  `redistrib_13.4.2.json` and `redistrib_9.26.0.json`, the GitHub release digest of ONNX
  Runtime 1.28.2 (CUDA 13), and the CUDA 13.3.1 compiler pieces that build mistral.rs (whose build
  accepts toolkits up to 13.3) into `cuda-13.3-build/`; `runtime_archives` lists every runtime
  archive.

## Boundaries

- Depends on: `ureq` (https with rustls), `sha2`, `lzma-rs`, `flate2`, `tar`; `std` for the files.
- Used by: `crates/inference/src/cuda_runtime/` (the folder names); the app's settings page
  (`apps/tbd_subtitles/src/settings/services/model_downloads.rs`); `tools/stack_spike/` (the
  `fetch` command and the model folders).
- Rules:
  - a file at its final path matches its pinned size and hash (`fetch_verified`; the
    `a_file_already_in_place_is_hashed_and_kept` test);
  - every pin is https with a 64-digit SHA-256 (`every_pin_is_https_with_a_sha256`);
  - no archive entry lands outside its folder
    (`strip_first_drops_the_top_folder_and_refuses_escapes`);
  - models are downloaded already exported and never converted (the crate header in
    `crates/inference/src/lib.rs`).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#models) — the models folder and
  its manifest.
- [Development environment](/documentation/runbooks/development_environment.md#cuda-libraries-for-onnx-runtime)
  — fetching and using the CUDA runtime.
