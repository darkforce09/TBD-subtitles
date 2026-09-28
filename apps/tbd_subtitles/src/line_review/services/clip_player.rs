//! Playing a line's clip: FFmpeg sends its sound to the desktop's sound server and a second FFmpeg
//! decodes small frames of its picture, shown in time with the sound; and a line's still frame,
//! decoded when the line opens so its picture shows before Play.
//!
//! **Role:** start both FFmpeg processes of a clip on a thread of their own, hand the newest frame
//! to the window at the clip's pace, say where the clip is, and stop both on request; decode one
//! still frame on a thread of its own.
//!
//! **Position:** called by the application's review actions; builds its command lines with
//! `media_io::preview` and runs them through `child_process`; the review's clip view reads
//! `PAD_S` and a clip's place.
//!
//! **Signals and state:** one thread per clip and per still; the clip's two FFmpeg children,
//! killed through a shared stop flag; the newest frame behind a mutex; the moment the sound
//! started; wakes the window at each frame and at the end.
//!
//! **Invariants:** the thread that starts the children waits for them, so they die with it and
//! never outlive the clip or the still; the video is only read; every frame carries a serial no
//! other frame of the window has; a clip's place never passes its end.

use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use child_process::Run;
use media_io::preview::{self, Clip};

use crate::core::background::Wake;
use crate::line_review::models::clip::{Frame, Sound};

/// Seconds of the video played before and after a line.
pub(crate) const PAD_S: f64 = 0.75;
/// Frames per second of the preview picture.
const FPS: u32 = 12;
/// Lines of the preview picture.
const LINES: u32 = 360;
/// The longest a clip's FFmpeg may run.
const DEADLINE: Duration = Duration::from_secs(120);
/// The longest a still frame's FFmpeg may run.
const STILL_DEADLINE: Duration = Duration::from_secs(10);

/// The serial of the next frame decoded.
static SERIAL: AtomicU32 = AtomicU32::new(0);

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
    /// When the sound's FFmpeg started.
    began: Arc<Mutex<Option<Instant>>>,
    clip: Clip,
    sound: Sound,
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

    pub(crate) fn sound(&self) -> Sound {
        self.sound
    }

    /// Where the clip is, in video seconds: its start plus the time since its sound began.
    pub(crate) fn position_s(&self) -> f64 {
        let began = self.began.lock().ok().and_then(|began| *began);
        position_at(self.clip, began, Instant::now())
    }
}

impl Drop for ClipPlayer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Where `clip` is at `now` when its sound `began`: its start plus the time since, capped at its
/// end; its start before the sound begins.
pub(crate) fn position_at(clip: Clip, began: Option<Instant>, now: Instant) -> f64 {
    let elapsed = began.map_or(0.0, |began| {
        now.saturating_duration_since(began).as_secs_f64()
    });
    clip.start_s + elapsed.min(clip.duration_s)
}

/// Start playing `request`; `wake` runs at each frame and when the clip ends.
pub(crate) fn play(request: ClipRequest, wake: Wake) -> ClipPlayer {
    let player = ClipPlayer {
        stop: Arc::new(AtomicBool::new(false)),
        playing: Arc::new(AtomicBool::new(true)),
        frame: Arc::new(Mutex::new(None)),
        began: Arc::new(Mutex::new(None)),
        clip: request.clip,
        sound: request.sound,
    };
    let (stop, playing, frame, began) = (
        player.stop.clone(),
        player.playing.clone(),
        player.frame.clone(),
        player.began.clone(),
    );
    std::thread::spawn(move || {
        if let Err(error) = run(&request, &stop, &frame, &began, &wake) {
            tracing::warn!(%error, "the clip could not play");
        }
        playing.store(false, Ordering::SeqCst);
        wake();
    });
    player
}

fn spawn(
    args: Vec<String>,
    deadline: Duration,
    stop: &Arc<AtomicBool>,
) -> Result<child_process::Running, String> {
    Run::new("ffmpeg")
        .args(args)
        .timeout(deadline)
        .cancel_on(stop.clone())
        .spawn()
        .map_err(|e| e.to_string())
}

fn run(
    request: &ClipRequest,
    stop: &Arc<AtomicBool>,
    slot: &Mutex<Option<Frame>>,
    began: &Mutex<Option<Instant>>,
    wake: &Wake,
) -> Result<(), String> {
    let sound_args = match request.sound {
        Sound::Mix => preview::track_sound(&request.video, request.audio_position, request.clip),
        Sound::Voices => preview::stem_sound(&request.vocals, request.clip),
    };
    let size = preview::frame_size(request.picture.0, request.picture.1, LINES);
    let frame_args = preview::frames(&request.video, request.clip, size, FPS);
    let started = Instant::now();
    let sound = spawn(sound_args, DEADLINE, stop)?;
    if let Ok(mut began) = began.lock() {
        *began = Some(Instant::now());
    }
    let mut pictures = spawn(frame_args, DEADLINE, stop)?;
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
                *frame = Some(frame_of(size, buffer.clone()));
            }
            wake();
            index += 1;
        }
    }
    let _ = pictures.wait();
    let _ = sound.wait();
    Ok(())
}

/// A frame of `size` holding `rgba`, with a serial of its own.
fn frame_of(size: (u32, u32), rgba: Vec<u8>) -> Frame {
    Frame {
        serial: SERIAL.fetch_add(1, Ordering::Relaxed),
        width: size.0,
        height: size.1,
        rgba,
    }
}

/// A line's still frame, decoding or decoded.
pub(crate) struct Still {
    stop: Arc<AtomicBool>,
    decoding: Arc<AtomicBool>,
    frame: Arc<Mutex<Option<Frame>>>,
}

impl Still {
    /// The frame, once decoded.
    pub(crate) fn frame(&self) -> Option<Frame> {
        self.frame.lock().ok().and_then(|frame| frame.clone())
    }

    pub(crate) fn is_decoding(&self) -> bool {
        self.decoding.load(Ordering::SeqCst)
    }
}

impl Drop for Still {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Decode the frame of `video` (its picture `picture` wide and high) at `at_s` on a thread;
/// `wake` runs when it is done.
pub(crate) fn still(video: PathBuf, picture: (u32, u32), at_s: f64, wake: Wake) -> Still {
    let still = Still {
        stop: Arc::new(AtomicBool::new(false)),
        decoding: Arc::new(AtomicBool::new(true)),
        frame: Arc::new(Mutex::new(None)),
    };
    let (stop, decoding, slot) = (
        still.stop.clone(),
        still.decoding.clone(),
        still.frame.clone(),
    );
    std::thread::spawn(move || {
        let size = preview::frame_size(picture.0, picture.1, LINES);
        let one_frame = Clip {
            start_s: at_s.max(0.0),
            duration_s: 1.0 / f64::from(FPS),
        };
        match spawn(
            preview::frames(&video, one_frame, size, FPS),
            STILL_DEADLINE,
            &stop,
        ) {
            Ok(mut pictures) => {
                if let Some(mut out) = pictures.take_stdout() {
                    let mut buffer = vec![0u8; (size.0 * size.1 * 4) as usize];
                    if out.read_exact(&mut buffer).is_ok()
                        && let Ok(mut frame) = slot.lock()
                    {
                        *frame = Some(frame_of(size, buffer));
                    }
                    // Whatever else it writes goes unread, so FFmpeg ends and is waited for.
                    let _ = std::io::copy(&mut out, &mut std::io::sink());
                }
                let _ = pictures.wait();
            }
            Err(error) => tracing::warn!(%error, "the still frame could not be decoded"),
        }
        decoding.store(false, Ordering::SeqCst);
        wake();
    });
    still
}

#[cfg(test)]
#[path = "tests/clip_player.rs"]
mod tests;
