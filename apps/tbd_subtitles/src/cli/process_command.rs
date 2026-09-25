//! `tbd-subtitles process <video>...`: one job per video, run to the end without a window.

use std::path::PathBuf;

use anyhow::{Context, bail};

/// Check every video is a readable file, then run its job.
pub(super) fn run(videos: &[PathBuf]) -> anyhow::Result<()> {
    for video in videos {
        let metadata = std::fs::metadata(video)
            .with_context(|| format!("cannot read the video {}", video.display()))?;
        if !metadata.is_file() {
            bail!("{} is not a file", video.display());
        }
    }
    bail!(
        "no pipeline stage is built yet, so no subtitles can be generated for {} video(s)",
        videos.len()
    )
}
