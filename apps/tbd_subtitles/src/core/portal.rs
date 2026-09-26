//! The desktop portal: the desktop's own file and folder chooser, and opening a file in the
//! desktop's default program (VLC for the owner's videos), over D-Bus.
//!
//! **Role:** ask the XDG desktop portal on a thread of its own, so the window keeps drawing while
//! the dialog is open, and hand the answer back through a channel.
//!
//! **Position:** called by the application for the queue's "Add videos…" and "Add folder…", the
//! settings' folder choosers and the report's "Open in VLC"; uses `ashpd` (zbus, pure Rust).
//!
//! **Signals and state:** one thread per request; D-Bus calls to `org.freedesktop.portal`.
//!
//! **Invariants:** the app starts no program itself: the desktop opens the file; a dialog the
//! owner closes answers with no paths, never an error.

use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

use ashpd::desktop::file_chooser::{FileFilter, SelectedFiles};
use ashpd::desktop::open_uri::OpenFileRequest;
use ashpd::desktop::{ResponseError, open_uri::OpenDirectoryRequest};

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

async fn ask(kind: Choose, title: &str) -> Chosen {
    let request = SelectedFiles::open_file().title(title).modal(true);
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

/// Ask the desktop to open `path` (a file in its default program, a folder in the file manager),
/// on a thread; a failure is logged.
pub(crate) fn open(path: &Path) {
    let path = path.to_path_buf();
    std::thread::spawn(move || {
        let result = pollster::block_on(async {
            if path.is_dir() {
                let folder = std::fs::File::open(&path).map_err(|e| e.to_string())?;
                OpenDirectoryRequest::default()
                    .send(&folder)
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            } else {
                let uri = ashpd::Uri::parse(&file_uri(&path)).map_err(|e| e.to_string())?;
                OpenFileRequest::default()
                    .send_uri(&uri)
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
        });
        if let Err(error) = result {
            tracing::warn!(path = %path.display(), %error, "the desktop could not open it");
        }
    });
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
