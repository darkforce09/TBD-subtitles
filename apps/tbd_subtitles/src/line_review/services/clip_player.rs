//! Playing a line's clip: FFmpeg sends its sound to the desktop's sound server and a second FFmpeg
//! decodes small frames of its picture, shown in time with the sound.
//!
//! **Role:** start both FFmpeg processes on a thread of their own, hand the newest frame to the
//! window at the clip's pace, and stop both on request.
//!
//! **Position:** called by the application's review actions; builds its command lines with
//! `media_io::preview` and runs them through `child_process`.
//!
//! **Signals and state:** one thread per clip; two FFmpeg children, killed through a shared stop
//! flag; the newest frame behind a mutex; wakes the window at each frame and at the end.
//!
//! **Invariants:** the thread that starts the children waits for both, so they die with it and
//! never outlive the clip; the video is only read.

use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use child_process::Run;
use media_io::preview::{self, Clip};

use crate::core::background::Wake;
use crate::line_review::models::clip::{Frame, Sound};

/// Frames per second of the preview picture.
const FPS: u32 = 12;
/// Lines of the preview picture.
const LINES: u32 = 360;
/// The longest a clip's FFmpeg may run.
const DEADLINE: Duration = Duration::from_secs(120);

/// What to play.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ClipRequest {
    pub(crate) video: PathBuf,
    pub(crate) audio_position: u32,
    /// The vocal stem, for `Sound::Voices`.
    pub(crate) vocals: PathBuf,
    pub(crate) picture: (u32, u32),
    pub(crate) clip: Clip,
    pub(crate) sound: Sound,
}

/// A clip playing, or played.
pub(crate) struct ClipPlayer {
    stop: Arc<AtomicBool>,
    playing: Arc<AtomicBool>,
    frame: Arc<Mutex<Option<Frame>>>,
}

impl ClipPlayer {
    pub(crate) fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.playing.load(Ordering::SeqCst)
    }

    /// The newest frame shown, if any.
    pub(crate) fn frame(&self) -> Option<Frame> {
        self.frame.lock().ok().and_then(|frame| frame.clone())
    }
}

impl Drop for ClipPlayer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Start playing `request`; `wake` runs at each frame and when the clip ends.
pub(crate) fn play(request: ClipRequest, wake: Wake) -> ClipPlayer {
    let player = ClipPlayer {
        stop: Arc::new(AtomicBool::new(false)),
        playing: Arc::new(AtomicBool::new(true)),
        frame: Arc::new(Mutex::new(None)),
    };
    let (stop, playing, frame) = (
        player.stop.clone(),
        player.playing.clone(),
        player.frame.clone(),
    );
    std::thread::spawn(move || {
        if let Err(error) = run(&request, &stop, &frame, &wake) {
            tracing::warn!(%error, "the clip could not play");
        }
        playing.store(false, Ordering::SeqCst);
        wake();
    });
    player
}

fn run(
    request: &ClipRequest,
    stop: &Arc<AtomicBool>,
    slot: &Mutex<Option<Frame>>,
    wake: &Wake,
) -> Result<(), String> {
    let sound_args = match request.sound {
        Sound::Mix => preview::track_sound(&request.video, request.audio_position, request.clip),
        Sound::Voices => preview::stem_sound(&request.vocals, request.clip),
    };
    let size = preview::frame_size(request.picture.0, request.picture.1, LINES);
    let frame_args = preview::frames(&request.video, request.clip, size, FPS);
    let spawn = |args: Vec<String>| {
        Run::new("ffmpeg")
            .args(args)
            .timeout(DEADLINE)
            .cancel_on(stop.clone())
            .spawn()
            .map_err(|e| e.to_string())
    };
    let sound = spawn(sound_args)?;
    let mut pictures = spawn(frame_args)?;
    let started = Instant::now();
    if let Some(mut out) = pictures.take_stdout() {
        let mut buffer = vec![0u8; (size.0 * size.1 * 4) as usize];
        let mut index = 0u32;
        while out.read_exact(&mut buffer).is_ok() {
            let due = started + Duration::from_secs_f64(f64::from(index) / f64::from(FPS));
            if let Some(wait) = due.checked_duration_since(Instant::now()) {
                std::thread::sleep(wait);
            }
            if stop.load(Ordering::SeqCst) {
                break;
            }
            if let Ok(mut frame) = slot.lock() {
                *frame = Some(Frame {
                    index,
                    width: size.0,
                    height: size.1,
                    rgba: buffer.clone(),
                });
            }
            wake();
            index += 1;
        }
    }
    let _ = pictures.wait();
    let _ = sound.wait();
    Ok(())
}
