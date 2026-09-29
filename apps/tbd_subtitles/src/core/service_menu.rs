//! The Dolphin service menu: "Generate subtitles" on a video's right-click menu.
//!
//! **Role:** write `kio/servicemenus/tbd-subtitles.desktop` under the user's data folder, whose
//! one action starts this AppImage as `process --enqueue` with the chosen videos, and copy the
//! AppImage's icon into the app's data folder for the menu to show.
//!
//! **Position:** part of `core`; the app calls `install_from_env` when it starts as an AppImage.
//! It reads `APPIMAGE` and `APPDIR` (set by the AppImage runtime), `XDG_DATA_HOME` and `HOME`.
//!
//! **Signals and state:** writes two files: the menu, executable (mode 0755, which KDE requires
//! of a service menu), and the icon; each only when its bytes differ, through a `.part` file
//! renamed into place.
//!
//! **Invariants:** the menu points at the AppImage that last started, so a re-imported AppImage
//! takes over the menu on its first start; an unchanged menu is never rewritten. The Exec line
//! follows the Desktop Entry specification: the program path is double-quoted with `"`, `` ` ``,
//! `$` and `\` backslash-escaped inside the quotes and `%` doubled, and the value is then escaped
//! as a desktop-entry string, so every backslash is doubled once more and a line break is `\n`.

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// The menu file's name under `kio/servicemenus`.
pub(crate) const MENU_FILE: &str = "tbd-subtitles.desktop";
/// The icon file the AppImage holds at its root, and its copy's name in the data folder.
const ICON_FILE: &str = "tbd-subtitles.png";
/// The menu's file mode: KDE runs only executable service menus.
const MENU_MODE: u32 = 0o755;

/// What an install did.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Installed {
    /// The menu was written at this path.
    Written(PathBuf),
    /// The menu at this path already said the same.
    Unchanged(PathBuf),
}

/// `$XDG_DATA_HOME/kio/servicemenus`, else `~/.local/share/kio/servicemenus`; none without a
/// home folder.
pub(crate) fn menu_dir() -> Option<PathBuf> {
    menu_dir_under(
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
    )
}

/// [`menu_dir`] with the two variables given.
fn menu_dir_under(data_home: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    let data = match data_home.filter(|dir| dir.is_absolute()) {
        Some(dir) => dir,
        None => home.filter(|dir| dir.is_absolute())?.join(".local/share"),
    };
    Some(data.join("kio").join("servicemenus"))
}

/// The service menu's text: one action for every video that starts `exe` to queue them.
pub(crate) fn menu_text(exe: &Path, icon: &Path) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Service\n\
         X-KDE-ServiceTypes=KonqPopupMenu/Plugin\n\
         MimeType=video/*;\n\
         Actions=generateSubtitles\n\
         \n\
         [Desktop Action generateSubtitles]\n\
         Name=Generate subtitles\n\
         Icon={}\n\
         Exec={} process --enqueue %F\n",
        string_escaped(&icon.to_string_lossy()),
        string_escaped(&quoted_argument(&exe.to_string_lossy())),
    )
}

/// One Exec argument in double quotes, with the characters the specification reserves inside
/// quotes backslash-escaped and `%` doubled so it is no field code.
fn quoted_argument(argument: &str) -> String {
    let mut quoted = String::with_capacity(argument.len() + 2);
    quoted.push('"');
    for character in argument.chars() {
        match character {
            '"' | '`' | '$' | '\\' => {
                quoted.push('\\');
                quoted.push(character);
            }
            '%' => quoted.push_str("%%"),
            _ => quoted.push(character),
        }
    }
    quoted.push('"');
    quoted
}

/// A value escaped as a desktop-entry string: backslash, line break, tab and carriage return.
fn string_escaped(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\t' => escaped.push_str("\\t"),
            '\r' => escaped.push_str("\\r"),
            _ => escaped.push(character),
        }
    }
    escaped
}

/// Write the menu into `menu_dir` for `appimage`, with the icon copied from `icon_source` into
/// `icon_dir` when given; the menu names `icon_dir/tbd-subtitles.png` either way.
pub(crate) fn install_into(
    menu_dir: &Path,
    appimage: &Path,
    icon_source: Option<&Path>,
    icon_dir: &Path,
) -> io::Result<Installed> {
    let icon = icon_dir.join(ICON_FILE);
    if let Some(source) = icon_source {
        fs::create_dir_all(icon_dir)?;
        let bytes = fs::read(source)?;
        if fs::read(&icon).ok().as_deref() != Some(bytes.as_slice()) {
            write_through_part(&icon, &bytes, None)?;
        }
    }
    fs::create_dir_all(menu_dir)?;
    let menu = menu_dir.join(MENU_FILE);
    let text = menu_text(appimage, &icon);
    if fs::read(&menu).ok().as_deref() == Some(text.as_bytes()) {
        if fs::metadata(&menu)?.permissions().mode() & 0o7777 != MENU_MODE {
            fs::set_permissions(&menu, fs::Permissions::from_mode(MENU_MODE))?;
        }
        return Ok(Installed::Unchanged(menu));
    }
    write_through_part(&menu, text.as_bytes(), Some(MENU_MODE))?;
    Ok(Installed::Written(menu))
}

/// Write `bytes` to `path` through `path.part`, with `mode` when given, and rename into place.
fn write_through_part(path: &Path, bytes: &[u8], mode: Option<u32>) -> io::Result<()> {
    let mut part = path.as_os_str().to_owned();
    part.push(".part");
    let part = PathBuf::from(part);
    fs::write(&part, bytes)?;
    if let Some(mode) = mode {
        fs::set_permissions(&part, fs::Permissions::from_mode(mode))?;
    }
    fs::rename(&part, path)
}

/// Install the menu for the running AppImage: none unless `APPIMAGE` names an existing file.
/// The icon comes from `$APPDIR/tbd-subtitles.png` and is copied into the app's data folder.
pub(crate) fn install_from_env() -> Option<io::Result<Installed>> {
    install_from(
        std::env::var_os("APPIMAGE").map(PathBuf::from),
        std::env::var_os("APPDIR").map(PathBuf::from),
    )
}

/// [`install_from_env`] with the two variables given.
fn install_from(
    appimage: Option<PathBuf>,
    appdir: Option<PathBuf>,
) -> Option<io::Result<Installed>> {
    let appimage = appimage.filter(|path| path.is_file())?;
    Some(install_for(&appimage, appdir))
}

/// Install for an AppImage known to exist, into the user's menu and data folders.
fn install_for(appimage: &Path, appdir: Option<PathBuf>) -> io::Result<Installed> {
    let menu_dir = menu_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "neither XDG_DATA_HOME nor HOME names a folder for the service menu",
        )
    })?;
    let icon_dir = inference::model_store::app_data_dir().map_err(io::Error::other)?;
    let icon_source = appdir
        .map(|dir| dir.join(ICON_FILE))
        .filter(|icon| icon.is_file());
    install_into(&menu_dir, appimage, icon_source.as_deref(), &icon_dir)
}

#[cfg(test)]
#[path = "tests/service_menu.rs"]
mod tests;
