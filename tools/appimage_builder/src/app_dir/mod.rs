//! The AppDir: the folder tree the AppImage's squashfs image holds.
//!
//! **Role:** start an empty AppDir, name its `usr/bin` and `usr/lib`, and write the parts every
//! AppImage needs at its root: the `.desktop` entry (also under `usr/share/applications`), the
//! icon (also under `usr/share/icons`), the `.DirIcon` symlink and the `AppRun` symlink to the
//! app binary, which opens the window when started with no arguments.
//!
//! **Position:** called by `main`, which fills `usr/` between `create` and `finish`; `icon` draws
//! the picture.
//!
//! **Signals and state:** writes under the AppDir path it is given; holds that path.
//!
//! **Invariants:** `create` leaves an empty folder, whatever was there; everything at the root is
//! generated here, so no packaging file is tracked in the repository.

mod icon;

pub(crate) use icon::icon_png;

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// The name the app, its desktop entry, its icon and its window class share.
pub(crate) const APP_NAME: &str = "tbd-subtitles";

/// The desktop entry: the window groups under the icon through `StartupWMClass`, which matches
/// the app's window class.
pub(crate) const DESKTOP_ENTRY: &str = "[Desktop Entry]
Type=Application
Name=TBD-subtitles
GenericName=Subtitle generator
Comment=Generate English subtitles for local videos
Exec=tbd-subtitles
Icon=tbd-subtitles
Categories=AudioVideo;Video;
StartupWMClass=tbd-subtitles
Terminal=false
";

/// The side of the square icon, in pixels.
pub(crate) const ICON_SIZE: u32 = 256;

/// An AppDir being laid out.
pub(crate) struct AppDir {
    root: PathBuf,
}

impl AppDir {
    /// An empty AppDir at `root`, removing whatever was there.
    pub(crate) fn create(root: &Path) -> Result<AppDir> {
        if fs::symlink_metadata(root).is_ok() {
            fs::remove_dir_all(root).with_context(|| format!("removing {}", root.display()))?;
        }
        let app_dir = AppDir {
            root: root.to_path_buf(),
        };
        for dir in [app_dir.usr_bin(), app_dir.usr_lib()] {
            fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        Ok(app_dir)
    }

    /// The AppDir's root.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// `usr/bin`: the two binaries, `cuda/` and `ffmpeg/`.
    pub(crate) fn usr_bin(&self) -> PathBuf {
        self.root.join("usr").join("bin")
    }

    /// `usr/lib`: the ggml worker's CrispASR and ggml libraries.
    pub(crate) fn usr_lib(&self) -> PathBuf {
        self.root.join("usr").join("lib")
    }

    /// Copy `binary` into `usr/bin` as `name`, executable; returns the copy's path.
    pub(crate) fn add_binary(&self, binary: &Path, name: &str) -> Result<PathBuf> {
        let target = self.usr_bin().join(name);
        fs::copy(binary, &target).with_context(|| format!("copying {}", binary.display()))?;
        fs::set_permissions(&target, fs::Permissions::from_mode(0o755))
            .with_context(|| format!("marking {} executable", target.display()))?;
        Ok(target)
    }

    /// Write the desktop entry, the icon and the root symlinks.
    pub(crate) fn finish(&self) -> Result<()> {
        let desktop = format!("{APP_NAME}.desktop");
        let icon = format!("{APP_NAME}.png");
        let png = icon_png(ICON_SIZE)?;
        let applications = self.root.join("usr/share/applications");
        let icons = self.root.join(format!(
            "usr/share/icons/hicolor/{ICON_SIZE}x{ICON_SIZE}/apps"
        ));
        for (dir, name, bytes) in [
            (self.root.clone(), &desktop, DESKTOP_ENTRY.as_bytes()),
            (applications, &desktop, DESKTOP_ENTRY.as_bytes()),
            (self.root.clone(), &icon, png.as_slice()),
            (icons, &icon, png.as_slice()),
        ] {
            fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
            let path = dir.join(name);
            fs::write(&path, bytes).with_context(|| format!("writing {}", path.display()))?;
        }
        for (link, points_at) in [
            (".DirIcon", icon.clone()),
            ("AppRun", format!("usr/bin/{APP_NAME}")),
        ] {
            let path = self.root.join(link);
            symlink(&points_at, &path).with_context(|| format!("linking {}", path.display()))?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/app_dir.rs"]
mod tests;
