//! One session thread of the pool: its screening session, then its confirming session.
//!
//! **Role:** open and warm up the thread's screening session (and, with the proxy pass on, its
//! proxy session), run the screening jobs the queue hands it at full resolution and at the proxy
//! width, merged, close both sessions when screening ends, open a confirming session the first
//! time a confirmation reaches the thread when it is a confirming thread, and send every result
//! back with its job's number.
//!
//! **Position:** spawned by `DetectorPool::open`, one per session; drains `queue.rs`.
//!
//! **Signals and state:** the thread's sessions; opening is serialised across threads by the
//! shared open lock, so engine builds and warm-ups never overlap and a second session reuses the
//! engine the first one built; times and notes go into the shared report.
//!
//! **Invariants:** every job taken from the queue gets exactly one answer, an error included,
//! even when its session failed to open or its run panicked, so the pool never waits forever.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, PoisonError};

use super::batch::{InputShape, check_frames, normalize_into};
use super::proxy;
use super::queue::{Queue, Task};
use super::regions::{PostProcess, Regions};
use super::session::{SessionOpener, SessionSpec, WarmSession, open_warm};
use crate::ocr::OcrError;
use crate::ocr::pool::{ConfirmResult, PaddedFrame, Priority, ScreenResult};

/// What the sessions report about their opening.
#[derive(Debug, Default)]
pub struct Report {
    pub warmup_s: f64,
    pub engine_build_s: f64,
    pub notes: BTreeMap<String, String>,
}

/// Everything a session thread shares with the pool.
pub struct Context {
    pub queue: Arc<Queue>,
    pub opener: Arc<dyn SessionOpener>,
    pub open_lock: Arc<Mutex<()>>,
    pub report: Arc<Mutex<Report>>,
    pub screen: SessionSpec,
    /// The proxy session's spec and the proxy frame size, when the proxy pass is on.
    pub proxy: Option<(SessionSpec, (u32, u32))>,
    pub confirm: SessionSpec,
    /// Whether this thread opens a confirming session once screening ends.
    pub confirms: bool,
    /// The frames' own width and height.
    pub frame: (u32, u32),
    pub screened: Sender<Result<ScreenResult, OcrError>>,
    pub confirmed: Sender<(usize, Result<ConfirmResult, OcrError>)>,
}

impl Context {
    /// Open and warm up `spec`'s session, one thread at a time, and report its times and notes.
    fn open(&self, spec: &SessionSpec, thread: usize) -> Result<WarmSession, OcrError> {
        let _opening = self
            .open_lock
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let warm = open_warm(self.opener.as_ref(), spec)?;
        let mut report = self.report.lock().unwrap_or_else(PoisonError::into_inner);
        report.warmup_s += warm.warmup_s;
        report.engine_build_s += warm.engine_build_s;
        for (key, value) in &warm.notes {
            report
                .notes
                .insert(format!("{key} (thread {})", thread + 1), value.clone());
        }
        Ok(warm)
    }
}

/// The regions of `frames`, run on `session` with input `shape`.
fn detect(
    session: &mut WarmSession,
    shape: InputShape,
    post: &PostProcess,
    frames: &[PaddedFrame],
    (width, height): (u32, u32),
) -> Result<Vec<Regions>, OcrError> {
    check_frames(frames, shape, width, height)?;
    normalize_into(frames, shape, session.session.staging())?;
    let mut found = None;
    session.session.run(&mut |maps| {
        found = Some(post.regions(maps, shape, frames.len(), (width, height))?);
        Ok(())
    })?;
    found.ok_or_else(|| "the detector run returned without its maps".into())
}

/// The regions of `frames` at full resolution, with those the proxy session finds on the frames
/// shrunk to the proxy size added where the full-resolution ones do not cover them.
fn screen_batch(
    context: &Context,
    sessions: &mut (WarmSession, Option<WarmSession>),
    post: &PostProcess,
    frames: &[PaddedFrame],
) -> Result<Vec<Regions>, OcrError> {
    let mut found = detect(
        &mut sessions.0,
        context.screen.input,
        post,
        frames,
        context.frame,
    )?;
    if let (Some(session), Some((spec, size))) = (sessions.1.as_mut(), context.proxy.as_ref()) {
        let small: Vec<PaddedFrame> = frames
            .iter()
            .map(|frame| proxy::shrink(frame, *size))
            .collect();
        let small_found = detect(session, spec.input, post, &small, *size)?;
        for (regions, extra) in found.iter_mut().zip(small_found) {
            proxy::merge(regions, proxy::scale_up(extra, *size, context.frame));
        }
    }
    Ok(found)
}

/// Open the thread's screening session and, with the proxy pass on, its proxy session.
fn open_screening(
    context: &Context,
    thread: usize,
) -> Result<(WarmSession, Option<WarmSession>), OcrError> {
    let screen = context.open(&context.screen, thread)?;
    let proxy = match &context.proxy {
        Some((spec, _)) => Some(context.open(spec, thread)?),
        None => None,
    };
    Ok((screen, proxy))
}

/// `work`'s result, with a panic turned into an error.
fn guarded<T>(work: impl FnOnce() -> Result<T, OcrError>) -> Result<T, OcrError> {
    catch_unwind(AssertUnwindSafe(work))
        .unwrap_or_else(|_| Err("the detector thread panicked while running a job".into()))
}

/// The body of session thread `thread`. `started` gets the screening session's opening result.
pub fn run(context: Context, thread: usize, started: Sender<Result<(), OcrError>>) {
    let mut screen = match guarded(|| open_screening(&context, thread)) {
        Ok(session) => {
            let _ = started.send(Ok(()));
            Some(session)
        }
        Err(error) => {
            context.queue.update(|state| state.screen_closed());
            let _ = started.send(Err(error));
            return;
        }
    };
    let screening = PostProcess::screening();
    let confirming = PostProcess::confirming();
    let mut confirm: Option<Result<WarmSession, String>> = None;
    loop {
        match context.queue.next(screen.is_some(), context.confirms) {
            Task::Exit => return,
            Task::CloseScreen => {
                screen = None;
                context.queue.update(|state| state.screen_closed());
            }
            Task::Screen(job) => {
                let result = guarded(|| {
                    let sessions = screen.as_mut().ok_or("the screening session is closed")?;
                    if job.priority == Priority::Probe {
                        detect(
                            &mut sessions.0,
                            context.screen.input,
                            &screening,
                            &job.frames,
                            context.frame,
                        )
                    } else {
                        screen_batch(&context, sessions, &screening, &job.frames)
                    }
                })
                .map(|regions| ScreenResult {
                    seq: job.seq,
                    regions,
                });
                let _ = context.screened.send(result);
            }
            Task::Confirm(index, job) => {
                let session = confirm.get_or_insert_with(|| {
                    guarded(|| context.open(&context.confirm, thread))
                        .map_err(|error| error.to_string())
                });
                let result = match session {
                    Err(error) => Err(error.clone().into()),
                    Ok(session) => guarded(|| {
                        detect(
                            session,
                            context.confirm.input,
                            &confirming,
                            std::slice::from_ref(&job.frame),
                            context.frame,
                        )
                    })
                    .and_then(|mut regions| regions.pop().ok_or_else(|| "no regions".into())),
                }
                .map(|regions| ConfirmResult {
                    seq: job.seq,
                    regions,
                });
                let _ = context.confirmed.send((index, result));
            }
        }
    }
}
