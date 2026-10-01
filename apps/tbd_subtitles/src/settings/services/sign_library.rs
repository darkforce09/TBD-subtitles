//! The sign library's size and its clearing, each on a thread of its own.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

use pipeline::library::Library;

use crate::core::background::Wake;

/// The signs the library at `path` holds and its file's bytes, or why they could not be read.
pub(crate) type Measured = Result<(u64, u64), String>;

/// The size of the library at `path`.
pub(crate) fn size(path: PathBuf) -> Measured {
    let size = Library::at(path)
        .size()
        .map_err(|error| error.to_string())?;
    Ok((size.signs, size.bytes))
}

/// Remove every sign of the library at `path`, then measure it.
pub(crate) fn clear(path: PathBuf) -> Measured {
    Library::at(path.clone())
        .clear()
        .map_err(|error| error.to_string())?;
    size(path)
}

/// Run `work` on the library at `path` on a thread; the answer arrives on the returned channel,
/// then `wake` runs.
pub(crate) fn start(
    path: PathBuf,
    work: fn(PathBuf) -> Measured,
    wake: Wake,
) -> Receiver<Measured> {
    let (send, answer) = channel();
    std::thread::spawn(move || {
        let _ = send.send(work(path));
        wake();
    });
    answer
}

#[cfg(test)]
#[path = "tests/sign_library.rs"]
mod tests;
