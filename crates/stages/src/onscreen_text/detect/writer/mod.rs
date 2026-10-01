//! One thread that writes the scan's crop and keyframe PNGs while confirmation goes on.
//!
//! **Role:** take crops and keyframe stills over a bounded channel, shrink keyframes to at most
//! 1280 pixels wide, and write each through the synced PNG writer.
//! **Position:** started by confirmation, which joins it before the scan returns.
//! **Signals and state:** one writer thread and a channel of at most `QUEUE` images.
//! **Invariants:** every PNG is written whole and synced before `finish` returns; the first
//! write error ends the thread and comes back from the next `send` or from `finish`, never lost;
//! the channel bounds the images waiting, so a slow disk holds confirmation back instead of
//! filling memory.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread::JoinHandle;

use image::{ImageFormat, RgbImage, imageops};

use crate::onscreen_text::{TextResult, png};

/// Images waiting for the writer at most.
const QUEUE: usize = 8;
/// The widest saved keyframe still, in pixels.
const KEYFRAME_WIDTH: u32 = 1280;

/// An image to write under the job folder.
pub(crate) enum PngJob {
    /// A rectified crop, written as it is.
    Crop { path: PathBuf, image: RgbImage },
    /// A keyframe still, written at most 1280 pixels wide.
    Keyframe { path: PathBuf, still: RgbImage },
}

/// The writer thread and the channel to it.
pub(crate) struct PngThread {
    jobs: Option<SyncSender<PngJob>>,
    thread: Option<JoinHandle<TextResult<()>>>,
}

impl PngThread {
    /// Starts the writer for paths relative to `root`.
    pub(crate) fn start(root: &Path) -> Self {
        let (jobs, received) = sync_channel(QUEUE);
        let root = root.to_path_buf();
        let thread = std::thread::Builder::new()
            .name("detect-png".to_string())
            .spawn(move || write_all(&root, &received))
            .expect("the PNG writer's thread starts");
        Self {
            jobs: Some(jobs),
            thread: Some(thread),
        }
    }

    /// Queues `job`; the writer's error when it has already failed.
    pub(crate) fn send(&mut self, job: PngJob) -> TextResult<()> {
        let sent = self
            .jobs
            .as_ref()
            .is_some_and(|jobs| jobs.send(job).is_ok());
        if sent {
            return Ok(());
        }
        self.join()?;
        Err("The PNG writer stopped before every image was written".into())
    }

    /// Waits until every queued image is written; the first write error, if any.
    pub(crate) fn finish(mut self) -> TextResult<()> {
        self.join()
    }

    fn join(&mut self) -> TextResult<()> {
        drop(self.jobs.take());
        match self.thread.take().map(JoinHandle::join) {
            Some(Ok(written)) => written,
            Some(Err(_)) => Err("The PNG writer thread panicked".into()),
            None => Ok(()),
        }
    }
}

impl Drop for PngThread {
    fn drop(&mut self) {
        let _ = self.join();
    }
}

/// Writes every job until the channel closes or a write fails.
fn write_all(root: &Path, jobs: &Receiver<PngJob>) -> TextResult<()> {
    for job in jobs {
        match job {
            PngJob::Crop { path, image } => png::write(&root.join(path), |out| {
                image.write_to(out, ImageFormat::Png)
            })?,
            PngJob::Keyframe { path, still } => save_keyframe(&still, &root.join(path))?,
        }
    }
    Ok(())
}

/// Saves a still at most 1280 pixels wide, keeping its aspect.
fn save_keyframe(still: &RgbImage, path: &Path) -> TextResult<()> {
    if still.width() <= KEYFRAME_WIDTH {
        return png::write(path, |out| still.write_to(out, ImageFormat::Png));
    }
    let height = (f64::from(still.height()) * f64::from(KEYFRAME_WIDTH) / f64::from(still.width()))
        .round()
        .max(1.0) as u32;
    let resized = imageops::resize(
        still,
        KEYFRAME_WIDTH,
        height,
        imageops::FilterType::Triangle,
    );
    png::write(path, |out| resized.write_to(out, ImageFormat::Png))
}

#[cfg(test)]
#[path = "tests/writer.rs"]
mod tests;
