//! One window per desktop session: a later start hands its videos to the running window.
//!
//! **Role:** decide whether this process is the first instance (`claim`), and if so accept
//! hand-offs from later ones on a Unix socket (`serve`); a later instance sends its videos and
//! wishes as one JSON line and waits for the answer (`hand_off`).
//!
//! **Position:** part of `core`; the command line claims before it opens a window, and the window
//! reads the hand-offs `serve` delivers. It reads `XDG_RUNTIME_DIR` and falls back to the model
//! store's data folder.
//!
//! **Signals and state:** `dir/instance.lock` carries an exclusive `flock` for as long as the
//! first instance's [`Instance`] lives; `dir/instance.sock` is its listening socket. `serve` runs
//! one named thread that owns the `Instance`, sends each valid hand-off on a channel and calls the
//! wake, when one is set, so the window draws a frame and reads it.
//!
//! **Invariants:** `flock` belongs to an open file description, so two opens of the lock file
//! conflict even inside one process, and the kernel drops the lock when the process dies, so a
//! crash never leaves a stale claim; a claim removes the socket file a dead instance left before
//! binding its own. Every connection gets exactly one reply line, `{"ok":true}` or
//! `{"ok":false,"error":…}`, and only a valid hand-off reaches the channel. The serving thread
//! ends at the first connection after the receiver is dropped, and the lock goes with it.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::core::background::Wake;

/// The file whose `flock` marks the first instance.
const LOCK_FILE: &str = "instance.lock";
/// The socket the first instance listens on.
const SOCKET_FILE: &str = "instance.sock";
/// The folder under `XDG_RUNTIME_DIR` that holds both.
const RUNTIME_FOLDER: &str = "tbd-subtitles";
/// The longest hand-off line the first instance reads.
const MAX_LINE: u64 = 1024 * 1024;
/// How long the first instance waits for a connection's line, and for its reply to be taken.
const READ_PATIENCE: Duration = Duration::from_secs(2);
/// How long a later instance waits for the reply once its line is sent.
const REPLY_PATIENCE: Duration = Duration::from_secs(5);
/// How often a later instance tries to connect while the first one binds its socket.
const RETRY_EVERY: Duration = Duration::from_millis(100);

/// What a later start asks of the running window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HandOff {
    /// The videos to add to the queue, as absolute paths.
    pub(crate) videos: Vec<PathBuf>,
    /// Whether the queue starts once they are added.
    pub(crate) start: bool,
    /// Whether the window comes to the front.
    pub(crate) raise: bool,
}

/// The answer the first instance writes back on each connection.
#[derive(Debug, Serialize, Deserialize)]
struct Reply {
    ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// The outcome of a claim.
#[derive(Debug)]
pub(crate) enum Claim {
    /// This process is the first instance and holds the lock and the socket.
    First(Instance),
    /// Another process holds the lock: hand off to it.
    Running,
}

/// The first instance's lock and listening socket; dropping it frees both.
#[derive(Debug)]
pub(crate) struct Instance {
    lock: File,
    listener: UnixListener,
    dir: PathBuf,
}

impl Drop for Instance {
    fn drop(&mut self) {
        // The socket goes before the lock, so a later claim never finds a live socket file.
        let _ = fs::remove_file(self.dir.join(SOCKET_FILE));
        let _ = self.lock.unlock();
    }
}

/// The folder for the lock and the socket: `$XDG_RUNTIME_DIR/tbd-subtitles`, created with mode
/// 0700, or the app's data folder when the runtime folder is unset.
pub(crate) fn instance_dir() -> anyhow::Result<PathBuf> {
    instance_dir_under(std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from))
}

/// [`instance_dir`] with the runtime folder given, so tests leave the environment alone.
fn instance_dir_under(runtime: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    match runtime.filter(|dir| dir.is_absolute()) {
        Some(runtime) => {
            let dir = runtime.join(RUNTIME_FOLDER);
            fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
                .with_context(|| format!("making {} private", dir.display()))?;
            Ok(dir)
        }
        None => {
            let dir = inference::model_store::app_data_dir()
                .context("finding the app's data folder for the instance lock")?;
            fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
            Ok(dir)
        }
    }
}

/// Take the instance lock in `dir`, which exists: `First` with the socket bound, or `Running`
/// when another process holds the lock.
pub(crate) fn claim(dir: &Path) -> io::Result<Claim> {
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join(LOCK_FILE))?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => return Ok(Claim::Running),
        Err(TryLockError::Error(error)) => return Err(error),
    }
    let socket = dir.join(SOCKET_FILE);
    match fs::remove_file(&socket) {
        Ok(()) => tracing::debug!(socket = %socket.display(), "removed a stale instance socket"),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let listener = UnixListener::bind(&socket)?;
    tracing::debug!(socket = %socket.display(), "this process is the first instance");
    Ok(Claim::First(Instance {
        lock,
        listener,
        dir: dir.to_path_buf(),
    }))
}

/// Accept hand-offs on a thread named `single-instance` that owns `instance`; each valid one
/// arrives on the returned channel, followed by a call to the wake when one is set. The thread
/// ends, freeing the lock and the socket, at the first connection after the receiver is dropped.
pub(crate) fn serve(instance: Instance, wake: Arc<OnceLock<Wake>>) -> Receiver<HandOff> {
    let (sender, receiver) = mpsc::channel();
    let spawned = thread::Builder::new()
        .name("single-instance".to_string())
        .spawn(move || accept_hand_offs(instance, &sender, &wake));
    if let Err(error) = spawned {
        tracing::error!(%error, "could not start the thread that takes later starts' videos");
    }
    receiver
}

/// The serving thread: one connection at a time, until the receiver is gone.
fn accept_hand_offs(instance: Instance, sender: &Sender<HandOff>, wake: &OnceLock<Wake>) {
    for connection in instance.listener.incoming() {
        let stream = match connection {
            Ok(stream) => stream,
            Err(error) => {
                tracing::warn!(%error, "a later start's connection failed");
                continue;
            }
        };
        let reply = match read_hand_off(&stream) {
            Ok(hand_off) => {
                let videos = hand_off.videos.len();
                if sender.send(hand_off).is_err() {
                    answer(&stream, &refusal("the window is closing"));
                    break;
                }
                tracing::info!(videos, "a later start handed its videos to this window");
                if let Some(wake) = wake.get() {
                    wake();
                }
                Reply {
                    ok: true,
                    error: None,
                }
            }
            Err(error) => {
                tracing::warn!(%error, "refused a later start's hand-off");
                refusal(&error)
            }
        };
        answer(&stream, &reply);
    }
    tracing::debug!(dir = %instance.dir.display(), "stopped taking later starts' videos");
}

/// A refusing reply.
fn refusal(error: &str) -> Reply {
    Reply {
        ok: false,
        error: Some(error.to_string()),
    }
}

/// Read and parse one hand-off line of at most [`MAX_LINE`] bytes.
fn read_hand_off(stream: &UnixStream) -> Result<HandOff, String> {
    stream
        .set_read_timeout(Some(READ_PATIENCE))
        .map_err(|error| format!("setting the read timeout: {error}"))?;
    let mut line = String::new();
    BufReader::new(stream)
        .take(MAX_LINE + 1)
        .read_line(&mut line)
        .map_err(|error| format!("reading the hand-off: {error}"))?;
    if line.len() as u64 > MAX_LINE {
        return Err(format!("the hand-off is longer than {MAX_LINE} bytes"));
    }
    serde_json::from_str(line.trim_end()).map_err(|error| format!("not a hand-off: {error}"))
}

/// Write one reply line; a later start that has gone away is only logged.
fn answer(mut stream: &UnixStream, reply: &Reply) {
    let mut line = serde_json::to_string(reply).unwrap_or_else(|_| r#"{"ok":false}"#.to_string());
    line.push('\n');
    let written = stream
        .set_write_timeout(Some(READ_PATIENCE))
        .and_then(|()| stream.write_all(line.as_bytes()));
    if let Err(error) = written {
        tracing::debug!(%error, "a later start left before its answer");
    }
}

/// Send `message` to the first instance in `dir`, its videos made absolute, and wait for its
/// answer; the connection is retried every 100 ms until `patience` runs out, which covers a first
/// instance that holds the lock but has not bound its socket.
pub(crate) fn hand_off(dir: &Path, message: &HandOff, patience: Duration) -> Result<(), String> {
    let videos = message
        .videos
        .iter()
        .map(|video| {
            std::path::absolute(video)
                .map_err(|error| format!("making {} absolute: {error}", video.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let message = HandOff {
        videos,
        ..message.clone()
    };
    let mut line = serde_json::to_string(&message)
        .map_err(|error| format!("writing the hand-off: {error}"))?;
    line.push('\n');
    let mut stream = connect(&dir.join(SOCKET_FILE), patience)?;
    stream
        .set_write_timeout(Some(REPLY_PATIENCE))
        .and_then(|()| stream.set_read_timeout(Some(REPLY_PATIENCE)))
        .and_then(|()| stream.write_all(line.as_bytes()))
        .map_err(|error| format!("sending the videos to the running window: {error}"))?;
    let mut reply = String::new();
    BufReader::new(&stream)
        .take(MAX_LINE)
        .read_line(&mut reply)
        .map_err(|error| format!("waiting for the running window's answer: {error}"))?;
    let reply: Reply = serde_json::from_str(reply.trim_end())
        .map_err(|error| format!("the running window's answer is not readable: {error}"))?;
    match reply {
        Reply { ok: true, .. } => Ok(()),
        Reply { error, .. } => Err(error.unwrap_or_else(|| "the running window refused".into())),
    }
}

/// Connect to `socket`, trying every [`RETRY_EVERY`] until `patience` runs out.
fn connect(socket: &Path, patience: Duration) -> Result<UnixStream, String> {
    let deadline = Instant::now() + patience;
    loop {
        match UnixStream::connect(socket) {
            Ok(stream) => return Ok(stream),
            Err(error) => {
                let now = Instant::now();
                if now >= deadline {
                    return Err(format!(
                        "no running window answered at {} within {:.1} s: {error}",
                        socket.display(),
                        patience.as_secs_f32()
                    ));
                }
                thread::sleep(RETRY_EVERY.min(deadline - now));
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/single_instance.rs"]
mod tests;
