//! Paired original and rendered playback for visual text review.
//!
//! **Role:** stream two RGB previews and optional sound while the window remains responsive.
//! **Position:** started by application actions; publishes plain pictures to the text-review view.
//! **Signals and state:** one background thread, three cancellable FFmpeg children and one frame slot.
//! **Invariants:** frames stay bounded at 720 pixels per edge; both pictures share time and size;
//! the exported ASS is rendered; dropping the player cancels all children.

use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use child_process::{Run, Running};
use media_io::Programs;
use media_io::preview::{
    self, Clip,
    visual::{self, VisualPreview},
};

use crate::core::background::Wake;
use crate::text_review::models::{Comparison, Picture, Session};

static SERIAL: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
struct State {
    comparison: Option<Comparison>,
    error: Option<String>,
}

pub(crate) struct Player {
    stop: Arc<AtomicBool>,
    playing: Arc<AtomicBool>,
    state: Arc<Mutex<State>>,
}

impl Player {
    pub(crate) fn comparison(&self) -> Option<Comparison> {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.comparison.clone())
    }

    pub(crate) fn error(&self) -> Option<String> {
        self.state.lock().ok().and_then(|state| state.error.clone())
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.playing.load(Ordering::SeqCst) && !self.stop.load(Ordering::SeqCst)
    }

    pub(crate) fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Decode from the cursor through the selected occurrence, or one still at the cursor.
pub(crate) fn start(session: &Session, time_s: f64, play: bool, wake: Wake) -> Player {
    let player = Player {
        stop: Arc::new(AtomicBool::new(false)),
        playing: Arc::new(AtomicBool::new(play)),
        state: Arc::new(Mutex::new(State::default())),
    };
    let dimensions = (session.document.width, session.document.height);
    let scale = 720.0 / f64::from(dimensions.0.max(dimensions.1).max(1));
    let size = (
        (f64::from(dimensions.0) * scale.min(1.0)).round().max(1.0) as u32,
        (f64::from(dimensions.1) * scale.min(1.0)).round().max(1.0) as u32,
    );
    let end = session
        .document
        .occurrences
        .get(session.selected)
        .map_or(session.duration_s, |text| text.end_s + 0.75)
        .max(time_s + 0.25)
        .min(session.duration_s);
    let request = VisualPreview {
        video: session.video.clone(),
        ass: Some(session.ass.clone()),
        time_s,
        duration_s: if play { (end - time_s).max(0.0) } else { 0.0 },
        size,
        fps: session.fps,
    };
    let audio = session.audio_position;
    let (stop, playing, state) = (
        player.stop.clone(),
        player.playing.clone(),
        player.state.clone(),
    );
    std::thread::spawn(move || {
        if let Err(error) = run(request, audio, play, &stop, &state, &wake)
            && !stop.load(Ordering::SeqCst)
        {
            tracing::warn!(%error, "the visual preview could not play");
            if let Ok(mut state) = state.lock() {
                state.error = Some(error);
            }
        }
        playing.store(false, Ordering::SeqCst);
        wake();
    });
    player
}

/// Assign a fresh identity to a thumbnail or preview frame, even after changing jobs.
pub(crate) fn picture(width: u32, height: u32, rgb: Vec<u8>) -> Picture {
    Picture {
        serial: SERIAL.fetch_add(1, Ordering::Relaxed),
        width,
        height,
        rgb,
    }
}

fn spawn(
    ffmpeg: &str,
    args: Vec<String>,
    deadline: Duration,
    stop: &Arc<AtomicBool>,
) -> Result<Running, String> {
    Run::new(ffmpeg)
        .args(args)
        .timeout(deadline)
        .cancel_on(stop.clone())
        .spawn()
        .map_err(|error| error.to_string())
}

fn run(
    request: VisualPreview,
    audio: u32,
    play: bool,
    stop: &Arc<AtomicBool>,
    state: &Mutex<State>,
    wake: &Wake,
) -> Result<(), String> {
    let rendered_args = visual::args(&request).map_err(|error| error.to_string())?;
    let mut original = request.clone();
    original.ass = None;
    let original_args = visual::args(&original).map_err(|error| error.to_string())?;
    let deadline = Duration::from_secs_f64(request.duration_s + 30.0);
    let ffmpeg = Programs::beside_current_exe().ffmpeg;
    let mut original = spawn(&ffmpeg, original_args, deadline, stop)?;
    let mut rendered = spawn(&ffmpeg, rendered_args, deadline, stop)?;
    let mut left = original
        .take_stdout()
        .ok_or("The original preview has no picture pipe.")?;
    let mut right = rendered
        .take_stdout()
        .ok_or("The rendered preview has no picture pipe.")?;
    let bytes = (request.size.0 * request.size.1 * 3) as usize;
    let mut left_frame = vec![0; bytes];
    let mut right_frame = vec![0; bytes];
    let mut index = 0;
    let mut began = None;
    let mut sound = None;
    loop {
        if stop.load(Ordering::SeqCst) {
            return Ok(());
        }
        let original_frame = read_frame(&mut left, &mut left_frame)?;
        let rendered_frame = read_frame(&mut right, &mut right_frame)?;
        if !original_frame || !rendered_frame {
            if original_frame != rendered_frame {
                return Err("Original and rendered previews ended on different frames.".into());
            }
            break;
        }
        if began.is_none() {
            if play && request.duration_s > 0.0 {
                sound = Some(spawn(
                    &ffmpeg,
                    preview::track_sound(
                        &request.video,
                        audio,
                        Clip {
                            start_s: request.time_s,
                            duration_s: request.duration_s,
                        },
                    ),
                    deadline,
                    stop,
                )?);
            }
            began = Some(Instant::now());
        }
        let offset = f64::from(index) / request.fps;
        let due = began.expect("first frame sets the clock") + Duration::from_secs_f64(offset);
        while let Some(wait) = due.checked_duration_since(Instant::now()) {
            if stop.load(Ordering::SeqCst) {
                return Ok(());
            }
            std::thread::sleep(wait.min(Duration::from_millis(10)));
        }
        if let Ok(mut state) = state.lock() {
            state.comparison = Some(Comparison {
                original: picture(request.size.0, request.size.1, left_frame.clone()),
                rendered: picture(request.size.0, request.size.1, right_frame.clone()),
                time_s: request.time_s + offset,
            });
        }
        wake();
        index += 1;
    }
    finish(original, "Original preview")?;
    finish(rendered, "Rendered preview")?;
    if let Some(sound) = sound {
        finish(sound, "Preview sound")?;
    }
    if index == 0 {
        return Err("No video frame is available at this position.".into());
    }
    Ok(())
}

fn read_frame(reader: &mut impl Read, buffer: &mut [u8]) -> Result<bool, String> {
    loop {
        match reader.read(&mut buffer[..1]) {
            Ok(0) => return Ok(false),
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(format!("Cannot read preview frame: {error}")),
        }
    }
    reader
        .read_exact(&mut buffer[1..])
        .map_err(|error| format!("Incomplete preview frame: {error}"))?;
    Ok(true)
}

fn finish(running: Running, what: &str) -> Result<(), String> {
    let result = running.wait().map_err(|error| format!("{what}: {error}"))?;
    if result.code != 0 {
        return Err(format!("{what}: {}", result.stderr.trim()));
    }
    Ok(())
}
