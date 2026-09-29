//! Timestamp-correct original and ASS-rendered pictures for visual text review.
//!
//! **Role:** build bounded RGB24 still or playback commands for FFmpeg.
//!
//! **Position:** called by the desktop's visual preview service; runs no child process itself.
//!
//! **Signals and state:** reads the request and returns separate command arguments.
//!
//! **Invariants:** the source stays read-only; subtitles render at native resolution and video
//! time before scaling; paths are escaped at both filter parsing levels, never as shell code.

use std::path::{Path, PathBuf};

use crate::MediaError;

/// One original or rendered view, beginning at a time relative to the video's start.
#[derive(Debug, Clone, PartialEq)]
pub struct VisualPreview {
    pub video: PathBuf,
    pub ass: Option<PathBuf>,
    pub time_s: f64,
    /// Zero requests one frame; a positive duration requests playback frames at `fps`.
    pub duration_s: f64,
    pub size: (u32, u32),
    pub fps: f64,
}

/// FFmpeg arguments yielding packed RGB24 frames on stdout, without invoking a shell.
///
/// Input seeking remains relative to the container's start (`seek_timestamp=0`), including
/// videos whose native timestamps do not begin at zero. The subtitle filter receives normalized
/// video time; downstream filters receive time relative to this preview's first frame.
pub fn args(request: &VisualPreview) -> Result<Vec<String>, MediaError> {
    validate(request)?;
    let mut filters = vec![format!("setpts=PTS-STARTPTS+{}/TB", request.time_s)];
    if let Some(path) = &request.ass {
        filters.push(format!("subtitles=filename={}", filter_path(path)?));
    }
    filters.push("setpts=PTS-STARTPTS".into());
    filters.push(format!("scale={}:{}", request.size.0, request.size.1));
    if request.duration_s > 0.0 {
        filters.push(format!("fps={}", request.fps));
    }
    let mut args = vec![
        "-nostdin".into(),
        "-hide_banner".into(),
        "-v".into(),
        "error".into(),
        "-filter_threads".into(),
        "1".into(),
        "-seek_timestamp".into(),
        "0".into(),
        "-ss".into(),
        request.time_s.to_string(),
        "-noautorotate".into(),
        "-i".into(),
        path_text(&request.video)?.to_owned(),
        "-map".into(),
        "0:v:0".into(),
        "-an".into(),
        "-sn".into(),
        "-dn".into(),
        "-vf".into(),
        filters.join(","),
    ];
    if request.duration_s > 0.0 {
        args.extend(["-t".into(), request.duration_s.to_string()]);
    } else {
        args.extend(["-frames:v".into(), "1".into()]);
    }
    args.extend([
        "-fps_mode".into(),
        "passthrough".into(),
        "-f".into(),
        "rawvideo".into(),
        "-pix_fmt".into(),
        "rgb24".into(),
        "pipe:1".into(),
    ]);
    Ok(args)
}

fn validate(request: &VisualPreview) -> Result<(), MediaError> {
    if !request.time_s.is_finite()
        || request.time_s < 0.0
        || !request.duration_s.is_finite()
        || request.duration_s < 0.0
        || !(request.time_s + request.duration_s).is_finite()
        || request.time_s + request.duration_s > i64::MAX as f64 / 1_000_000.0
    {
        return Err(MediaError::Parse(
            "preview times must be finite, nonnegative seconds".into(),
        ));
    }
    if !request.fps.is_finite() || request.fps <= 0.0 {
        return Err(MediaError::Parse(
            "preview frame rate must be finite and positive".into(),
        ));
    }
    let (width, height) = request.size;
    if width == 0
        || height == 0
        || width > i32::MAX as u32
        || height > i32::MAX as u32
        || width
            .checked_mul(height)
            .and_then(|size| size.checked_mul(3))
            .is_none()
    {
        return Err(MediaError::Parse(
            "preview dimensions must be positive and fit an RGB frame".into(),
        ));
    }
    Ok(())
}

fn path_text(path: &Path) -> Result<&str, MediaError> {
    let text = path
        .to_str()
        .ok_or_else(|| MediaError::Parse("preview paths must be valid UTF-8".into()))?;
    if text.is_empty() || text.contains('\0') {
        return Err(MediaError::Parse(
            "preview paths must be nonempty and contain no NUL".into(),
        ));
    }
    Ok(text)
}

/// Escape the option parser first and then the enclosing filtergraph parser. Whitespace is
/// protected at both levels because unquoted leading and trailing whitespace is otherwise lost.
fn filter_path(path: &Path) -> Result<String, MediaError> {
    let option = escape(path_text(path)?, ":\\'");
    Ok(escape(&option, "\\'[],;"))
}

fn escape(text: &str, special: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        if special.contains(character) || character.is_whitespace() {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

#[cfg(test)]
#[path = "tests/visual.rs"]
mod tests;
