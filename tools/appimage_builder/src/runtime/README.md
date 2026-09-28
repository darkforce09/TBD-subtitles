# AppImage runtime and output

The pinned AppImage type 2 runtime that heads the file and mounts the image behind it, and the
names and mode of the finished file in `dist/`.

## Contents

```text
tools/appimage_builder/src/runtime/
├── mod.rs  the pin and `fetch`, the output names, the app version and commit, and `finish`
└── tests/  the names, the version line, and the mode and stable copy `finish` leaves
```

## Boundaries

- Depends on: `inference::model_store::fetch_verified`, `child_process::Run` (for `git`),
  `gpu_runtime::print_progress`.
- Used by: `main.rs`: `fetch` before the AppDir, the names and `finish` after the image.
- Rules:
  - the runtime's SHA-256 matches the pin before it is used;
  - the finished AppImage is mode 755 and also sits under the stable name
    (`finishing_marks_executable_and_copies_to_the_stable_name`).
