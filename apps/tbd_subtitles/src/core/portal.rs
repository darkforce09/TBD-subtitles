//! The desktop portal: the desktop's own file and folder chooser, opening a file in the
//! desktop's default program (VLC for the owner's videos), and showing a file in the file
//! manager, over D-Bus.
//!
//! **Role:** ask the XDG desktop portal on a thread of its own, so the window keeps drawing while
//! the dialog is open, and hand the answer back through a channel.
//!
//! **Position:** called by the application for the toolbar's "Add Videos…" and "Add Folder…",
//! the settings' folder choosers, and the Overview's and the row menu's "Open in Player", "Show
//! in Folder" and "Open Full Report"; uses `ashpd` (zbus, pure Rust).
//!
//! **Signals and state:** one thread and one D-Bus session connection per request; calls to
//! `org.freedesktop.portal`. The connection closes when the request ends.
//!
//! **Invariants:** the app starts no program itself: the desktop opens the file; a dialog the
//! owner closes answers with no paths, never an error; an open that fails answers with why.
//! No request shares a connection with another request or with the colour scheme watcher, so a
//! request the desktop never answers holds up no other.

use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

use ashpd::desktop::file_chooser::{FileFilter, SelectedFiles};
use ashpd::desktop::open_uri::OpenFileRequest;
use ashpd::desktop::{ResponseError, open_uri::OpenDirectoryRequest};
use ashpd::zbus::Connection;

use crate::core::background::Wake;

/// What the owner chose, or why the portal could not ask.
pub(crate) type Chosen = Result<Vec<PathBuf>, String>;

/// Which chooser to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Choose {
    /// One or more video files.
    Videos,
    /// One folder.
    Folder,
    /// One JSON file (a glossary).
    Json,
}

/// Show the chooser on a thread; the answer arrives on the returned channel, then `wake` runs.
pub(crate) fn choose(kind: Choose, title: &str, wake: Wake) -> Receiver<Chosen> {
    let (send, answer) = channel();
    let title = title.to_string();
    std::thread::spawn(move || {
        let chosen = pollster::block_on(ask(kind, &title));
        let _ = send.send(chosen);
        wake();
    });
    answer
}

/// A session bus connection of the request's own. ashpd's shared connection holds a lock while
/// it waits for the bus, so one request that never returns there would stall every later one.
async fn connection() -> Result<Connection, String> {
    Connection::session().await.map_err(|e| e.to_string())
}

async fn ask(kind: Choose, title: &str) -> Chosen {
    let request = SelectedFiles::open_file()
        .connection(Some(connection().await?))
        .title(title)
        .modal(true);
    let request = match kind {
        Choose::Videos => request.multiple(true).filter(
            FileFilter::new("Videos")
                .mimetype("video/*")
                .glob("*.mkv")
                .glob("*.mp4"),
        ),
        Choose::Folder => request.directory(true),
        Choose::Json => request.filter(FileFilter::new("JSON").glob("*.json")),
    };
    let sent = request.send().await.map_err(|e| e.to_string())?;
    match sent.response() {
        Ok(files) => Ok(files
            .uris()
            .iter()
            .filter_map(|uri| file_path(uri.as_str()))
            .collect()),
        Err(ashpd::Error::Response(ResponseError::Cancelled)) => Ok(Vec::new()),
        Err(error) => Err(error.to_string()),
    }
}

/// Whether the desktop did what it was asked, or why not.
pub(crate) type Opened = Result<(), String>;

/// Ask the desktop to open `path` (a file in its default program, a folder in the file manager),
/// on a thread; the answer arrives on the returned channel, then `wake` runs.
pub(crate) fn open(path: &Path, wake: Wake) -> Receiver<Opened> {
    let (send, answer) = channel();
    let path = path.to_path_buf();
    std::thread::spawn(move || {
        let result = pollster::block_on(async {
            let connection = connection().await?;
            if path.is_dir() {
                let folder = std::fs::File::open(&path).map_err(|e| e.to_string())?;
                OpenDirectoryRequest::default()
                    .connection(Some(connection))
                    .send(&folder)
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            } else {
                let uri = ashpd::Uri::parse(&file_uri(&path)).map_err(|e| e.to_string())?;
                OpenFileRequest::default()
                    .connection(Some(connection))
                    .send_uri(&uri)
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
        });
        if let Err(error) = &result {
            tracing::warn!(path = %path.display(), %error, "the desktop could not open it");
        }
        let _ = send.send(result);
        wake();
    });
    answer
}

/// Ask the file manager to show `path` in the folder that holds it, on a thread; the answer
/// arrives on the returned channel, then `wake` runs.
pub(crate) fn reveal(path: &Path, wake: Wake) -> Receiver<Opened> {
    let (send, answer) = channel();
    let path = path.to_path_buf();
    std::thread::spawn(move || {
        let result = pollster::block_on(async {
            let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
            OpenDirectoryRequest::default()
                .connection(Some(connection().await?))
                .send(&file)
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
        });
        if let Err(error) = &result {
            tracing::warn!(path = %path.display(), %error, "the file manager could not show it");
        }
        let _ = send.send(result);
        wake();
    });
    answer
}

/// `file://` and the path, every byte outside the unreserved set and `/` percent-encoded.
pub(crate) fn file_uri(path: &Path) -> String {
    let mut uri = String::from("file://");
    for byte in path.as_os_str().as_encoded_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                uri.push(*byte as char)
            }
            _ => uri.push_str(&format!("%{byte:02X}")),
        }
    }
    uri
}

/// The local path a `file://` URI names, percent-decoded; `None` for another scheme or host.
pub(crate) fn file_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    if !rest.starts_with('/') {
        return None;
    }
    let bytes = rest.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(hex) = rest.get(i + 1..i + 3)
            && let Ok(byte) = u8::from_str_radix(hex, 16)
        {
            decoded.push(byte);
            i += 3;
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    Some(PathBuf::from(std::ffi::OsString::from_vec(decoded)))
}

#[cfg(test)]
#[path = "tests/portal.rs"]
mod tests;
