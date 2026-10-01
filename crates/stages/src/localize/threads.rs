//! Decode → blend → encode as three threads joined by bounded queues.
//!
//! **Role:** move a run of frames from a decoder to an encoder: the step thread takes each
//! decoded frame, blends its patches in place and hands it to an encoder thread over a bounded
//! channel; the encoder thread starts the encoder, writes every frame it is handed and finishes
//! it. Each thread's waiting and working is timed into `RenderPhases`.
//! **Position:** inside `localize`; the whole-video encode runs it once and the segment encode
//! once per re-encoded piece. The decode thread is the `FrameQueue` behind the frame source.
//! **Signals and state:** one scoped encoder thread per run and a channel of
//! `ENCODE_QUEUE_FRAMES` frames; an abort flag the step thread raises before it stops feeding.
//! **Invariants:** the encoder is started, written and finished on its own thread, which lives
//! until the encoder is reaped; frames reach it in the order decoded, each exactly once; a frame
//! whose index is not the next expected one is an error; an error on any thread stops the run
//! and comes back from it, the encoder's exit explaining a write it refused; a stopped run kills
//! the encoder rather than finishing a partial file.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, sync_channel};
use std::time::Instant;

use media_io::MediaError;
use media_io::encode::EncoderProcess;

use super::frames::RawFrame;
use super::{LocalizeError, LocalizeResult, RenderPhases};

/// Frames blended and waiting for the encoder: enough to ride out a slow frame on either side.
pub const ENCODE_QUEUE_FRAMES: usize = 8;

/// Where blended frames go: an encoder fed one whole frame at a time, then closed.
pub trait FrameSink {
    /// Take one frame; an error means the sink stopped reading, and `finish` says why.
    fn write_frame(&mut self, frame: &[u8]) -> Result<(), MediaError>;

    /// Close the sink and wait for it: the frames it took, or why it failed.
    fn finish(self) -> Result<u64, MediaError>;
}

impl FrameSink for EncoderProcess {
    fn write_frame(&mut self, frame: &[u8]) -> Result<(), MediaError> {
        EncoderProcess::write_frame(self, frame)
    }

    fn finish(self) -> Result<u64, MediaError> {
        EncoderProcess::finish(self)
    }
}

/// The frames one run moves: `count` frames from presentation index `first`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameRun {
    pub first: u64,
    pub count: u64,
}

/// Why the step thread stopped feeding the encoder.
enum Stop {
    /// The encoder thread hung up: its own result says why.
    EncoderGone,
    /// The decoder, the blend or a cancel stopped the run.
    Failed(LocalizeError),
}

/// Move `run`'s frames from `decode` through `blend` into the sink `start` opens on the encoder
/// thread, adding each thread's time to `phases`; `done` hears each frame handed over. The frames
/// the sink took, which equal `run.count` on success.
pub fn run_frames<S, Start>(
    run: FrameRun,
    decode: &mut dyn FnMut() -> Result<Option<RawFrame>, MediaError>,
    blend: &mut dyn FnMut(u64, &mut [u8]) -> LocalizeResult<()>,
    start: Start,
    cancel: Option<&AtomicBool>,
    phases: &mut RenderPhases,
    done: &mut dyn FnMut(),
) -> LocalizeResult<u64>
where
    S: FrameSink,
    Start: FnOnce() -> Result<S, MediaError> + Send,
{
    let abort = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let (frames, taken) = sync_channel::<RawFrame>(ENCODE_QUEUE_FRAMES);
        let encoder = std::thread::Builder::new()
            .name("localized-encode".into())
            .spawn_scoped(scope, || encode(start, taken, &abort))
            .map_err(|error| LocalizeError(format!("start the encoder thread: {error}")))?;
        let fed = (|| {
            for offset in 0..run.count {
                let expected = run.first + offset;
                let waited = Instant::now();
                let next = decode();
                phases.decode_wait += waited.elapsed();
                let mut frame = match next {
                    Ok(Some(frame)) => frame,
                    Ok(None) => {
                        return Err(Stop::Failed(
                            format!("the decoder ended before frame {expected}").into(),
                        ));
                    }
                    Err(error) => return Err(Stop::Failed(error.into())),
                };
                if cancel.is_some_and(|flag| flag.load(Ordering::SeqCst)) {
                    return Err(Stop::Failed("the localized video was cancelled".into()));
                }
                if frame.index != expected {
                    return Err(Stop::Failed(
                        format!("the decoder gave frame {} for {expected}", frame.index).into(),
                    ));
                }
                let blending = Instant::now();
                blend(frame.index, &mut frame.data).map_err(Stop::Failed)?;
                phases.blend += blending.elapsed();
                let handing = Instant::now();
                frames.send(frame).map_err(|_| Stop::EncoderGone)?;
                phases.encode_wait += handing.elapsed();
                done();
            }
            Ok(())
        })();
        let flushing = Instant::now();
        if fed.is_err() {
            abort.store(true, Ordering::SeqCst);
        }
        drop(frames);
        let encoded = encoder
            .join()
            .unwrap_or_else(|_| Err("the encoder thread panicked".into()));
        phases.flush += flushing.elapsed();
        match (fed, encoded) {
            (Err(Stop::Failed(error)), _) => Err(error),
            (Err(Stop::EncoderGone), Err(error)) => Err(error),
            (Err(Stop::EncoderGone), Ok(written)) => Err(format!(
                "the encoder stopped taking frames after {written} of {}",
                run.count
            )
            .into()),
            (Ok(()), Err(error)) => Err(error),
            (Ok(()), Ok(written)) if written != run.count => {
                Err(format!("the encoder took {written} frames of {}", run.count).into())
            }
            (Ok(()), Ok(written)) => Ok(written),
        }
    })
}

/// The encoder thread: start the sink, write every frame handed over until the channel closes,
/// then finish it, or kill it when the step thread aborted.
fn encode<S: FrameSink>(
    start: impl FnOnce() -> Result<S, MediaError>,
    frames: Receiver<RawFrame>,
    abort: &AtomicBool,
) -> LocalizeResult<u64> {
    let mut sink = start()?;
    for frame in frames.iter() {
        if let Err(error) = sink.write_frame(&frame.data) {
            drop(frames);
            // The sink stopped reading; its exit says why.
            return Err(match sink.finish() {
                Err(exit) => exit.into(),
                Ok(_) => error.into(),
            });
        }
    }
    if abort.load(Ordering::SeqCst) {
        drop(sink);
        return Err("the encode was stopped".into());
    }
    Ok(sink.finish()?)
}

#[cfg(test)]
#[path = "tests/threads.rs"]
mod tests;
