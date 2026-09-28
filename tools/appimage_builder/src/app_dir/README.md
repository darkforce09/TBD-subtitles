# AppDir layout

The folder tree the AppImage holds, and the root parts every AppImage needs, all generated: the
desktop entry, the icon, `.DirIcon` and the `AppRun` symlink to the app.

## Contents

```text
tools/appimage_builder/src/app_dir/
├── icon.rs  the icon, drawn in code: a rounded dark tile with two white subtitle bars, as a PNG
├── mod.rs   `AppDir`: create, `usr/bin`, `usr/lib`, add a binary, and the desktop entry and links
└── tests/   the desktop entry, the icon's PNG and the laid-out root
```

## Boundaries

- Depends on: `png` (in `icon.rs`), `anyhow`, the standard library's file system.
- Used by: `main.rs`, which creates the AppDir, fills `usr/` and calls `finish`.
- Rules:
  - `StartupWMClass` and `Icon` match the app's window class `tbd-subtitles`
    (`the_desktop_entry_names_the_app_icon_and_window_class`);
  - `AppRun` is a symlink, never a script (`finish_lays_out_the_root_of_the_app_dir`);
  - the icon's corners are transparent and its bytes are the same every run
    (`the_icon_is_a_square_png_with_transparent_corners`).
