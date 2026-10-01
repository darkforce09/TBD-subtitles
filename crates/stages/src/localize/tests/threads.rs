use std::sync::{Arc, Mutex};

use media_io::frame_queue::{FrameQueue, PooledBuffer, Producer};

use super::*;

/// What a fake sink saw: the first byte of every frame written, whether it was finished, and on
/// which thread it was started, written and finished.
#[derive(Debug, Default)]
struct Seen {
    frames: Vec<u8>,
    finished: bool,
    threads: Vec<std::thread::ThreadId>,
}

/// An encoder that records its frames and may refuse a frame or its finish.
struct FakeSink {
    seen: Arc<Mutex<Seen>>,
    refuse_at: Option<usize>,
    exit_error: Option<&'static str>,
}

impl FrameSink for FakeSink {
    fn write_frame(&mut self, frame: &[u8]) -> Result<(), MediaError> {
        let mut seen = self.seen.lock().unwrap();
        seen.threads.push(std::thread::current().id());
        if self.refuse_at == Some(seen.frames.len()) {
            return Err(MediaError::Parse("the pipe closed".into()));
        }
        seen.frames.push(frame[0]);
        Ok(())
    }

    fn finish(self) -> Result<u64, MediaError> {
        let mut seen = self.seen.lock().unwrap();
        seen.threads.push(std::thread::current().id());
        seen.finished = true;
        match self.exit_error {
            Some(error) => Err(MediaError::Parse(error.into())),
            None => Ok(seen.frames.len() as u64),
        }
    }
}

fn frame(index: u64) -> RawFrame {
    RawFrame {
        index,
        data: PooledBuffer::detached(vec![index as u8; 4]),
    }
}

/// A decoder of frames `first..end` that fails at `fail_at` when given.
fn frames(first: u64, end: u64, fail_at: Option<u64>) -> impl FnMut() -> DecodeResult {
    let mut next = first;
    move || {
        if fail_at == Some(next) {
            return Err(MediaError::Parse(format!("bad packet at {next}")));
        }
        if next == end {
            return Ok(None);
        }
        next += 1;
        Ok(Some(frame(next - 1)))
    }
}

type DecodeResult = Result<Option<RawFrame>, MediaError>;

struct Outcome {
    result: LocalizeResult<u64>,
    seen: Seen,
    blended: Vec<u64>,
    done: usize,
}

/// Run `count` frames from `first` through a blend that adds 100 to each frame's first byte and
/// fails at `blend_fails_at`, into a fake sink.
fn run(
    first: u64,
    count: u64,
    decode: &mut dyn FnMut() -> DecodeResult,
    sink: impl FnOnce(Arc<Mutex<Seen>>) -> Result<FakeSink, MediaError> + Send,
    blend_fails_at: Option<u64>,
    cancel: Option<&AtomicBool>,
) -> Outcome {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let mut blended = Vec::new();
    let mut done = 0;
    let mut phases = RenderPhases::default();
    let shared = seen.clone();
    let result = run_frames(
        FrameRun { first, count },
        decode,
        &mut |index, data| {
            if blend_fails_at == Some(index) {
                return Err("a patch is missing".into());
            }
            blended.push(index);
            data[0] = data[0].wrapping_add(100);
            Ok(())
        },
        move || sink(shared),
        cancel,
        &mut phases,
        &mut || done += 1,
    );
    let seen = std::mem::take(&mut *seen.lock().unwrap());
    Outcome {
        result,
        seen,
        blended,
        done,
    }
}

fn working(seen: Arc<Mutex<Seen>>) -> Result<FakeSink, MediaError> {
    seen.lock()
        .unwrap()
        .threads
        .push(std::thread::current().id());
    Ok(FakeSink {
        seen,
        refuse_at: None,
        exit_error: None,
    })
}

#[test]
fn frames_reach_the_encoder_blended_in_order_on_one_thread_of_its_own() {
    let outcome = run(5, 20, &mut frames(5, 25, None), working, None, None);
    assert_eq!(outcome.result, Ok(20));
    let expected: Vec<u8> = (5..25).map(|index| index as u8 + 100).collect();
    assert_eq!(outcome.seen.frames, expected);
    assert_eq!(outcome.blended, (5..25).collect::<Vec<u64>>());
    assert_eq!(outcome.done, 20);
    assert!(outcome.seen.finished);
    let encoder_thread = outcome.seen.threads[0];
    assert_ne!(encoder_thread, std::thread::current().id());
    assert!(
        outcome
            .seen
            .threads
            .iter()
            .all(|thread| *thread == encoder_thread),
        "the encoder is started, written and finished on one thread"
    );
}

#[test]
fn a_run_stops_after_its_count_and_leaves_the_rest_to_the_decoder() {
    let mut decode = frames(0, 100, None);
    let outcome = run(0, 10, &mut decode, working, None, None);
    assert_eq!(outcome.result, Ok(10));
    let next = decode().unwrap().unwrap();
    assert_eq!(
        next.index, 10,
        "the next run reads on from the same decoder"
    );
}

#[test]
fn a_decode_error_stops_the_run_and_kills_the_encoder() {
    let outcome = run(0, 10, &mut frames(0, 10, Some(4)), working, None, None);
    let error = outcome.result.unwrap_err();
    assert!(error.0.contains("bad packet at 4"), "{error}");
    assert_eq!(outcome.seen.frames.len(), 4);
    assert!(!outcome.seen.finished, "a stopped encode is never finished");
}

#[test]
fn a_decoder_that_ends_early_or_skips_a_frame_is_an_error() {
    let outcome = run(0, 10, &mut frames(0, 6, None), working, None, None);
    let error = outcome.result.unwrap_err();
    assert!(error.0.contains("ended before frame 6"), "{error}");
    let outcome = run(3, 10, &mut frames(4, 20, None), working, None, None);
    let error = outcome.result.unwrap_err();
    assert!(error.0.contains("frame 4 for 3"), "{error}");
    assert!(!outcome.seen.finished);
}

#[test]
fn a_blend_error_stops_the_run() {
    let outcome = run(0, 10, &mut frames(0, 10, None), working, Some(7), None);
    let error = outcome.result.unwrap_err();
    assert_eq!(error.0, "a patch is missing");
    assert!(!outcome.seen.finished);
}

#[test]
fn an_encoder_that_fails_to_start_or_stops_reading_says_why() {
    let outcome = run(
        0,
        10,
        &mut frames(0, 10, None),
        |_| Err(MediaError::Parse("no such encoder".into())),
        None,
        None,
    );
    assert!(outcome.result.unwrap_err().0.contains("no such encoder"));
    let outcome = run(
        0,
        50,
        &mut frames(0, 50, None),
        |seen| {
            Ok(FakeSink {
                seen,
                refuse_at: Some(3),
                exit_error: Some("ffmpeg exited with code 1: invalid level"),
            })
        },
        None,
        None,
    );
    let error = outcome.result.unwrap_err();
    assert!(
        error.0.contains("invalid level"),
        "the exit explains: {error}"
    );
    assert_eq!(outcome.seen.frames.len(), 3);
}

#[test]
fn an_encoder_that_fails_at_its_finish_fails_the_run() {
    let outcome = run(
        0,
        5,
        &mut frames(0, 5, None),
        |seen| {
            Ok(FakeSink {
                seen,
                refuse_at: None,
                exit_error: Some("muxer failed"),
            })
        },
        None,
        None,
    );
    assert!(outcome.result.unwrap_err().0.contains("muxer failed"));
}

#[test]
fn a_cancel_stops_the_run_early() {
    let cancel = AtomicBool::new(true);
    let outcome = run(
        0,
        10,
        &mut frames(0, 10, None),
        working,
        None,
        Some(&cancel),
    );
    let error = outcome.result.unwrap_err();
    assert!(error.0.contains("cancelled"), "{error}");
    assert!(outcome.seen.frames.is_empty());
    assert!(!outcome.seen.finished);
}

/// A producer on a real decode thread that fails at frame `fail_at` when given.
struct FakeProducer {
    next: u64,
    end: u64,
    fail_at: Option<u64>,
}

impl Producer for FakeProducer {
    type Item = RawFrame;
    type Error = MediaError;

    fn next(&mut self) -> Result<Option<RawFrame>, MediaError> {
        if self.fail_at == Some(self.next) {
            return Err(MediaError::Parse("the decoder crashed".into()));
        }
        if self.next == self.end {
            return Ok(None);
        }
        self.next += 1;
        Ok(Some(frame(self.next - 1)))
    }

    fn finish(self, _completed: bool) -> Result<(), MediaError> {
        Ok(())
    }
}

#[test]
fn frames_cross_from_a_decode_thread_and_its_errors_come_back() {
    let mut queue = FrameQueue::spawn(
        FakeProducer {
            next: 0,
            end: 300,
            fail_at: None,
        },
        4,
    );
    let outcome = run(0, 300, &mut || queue.recv(), working, None, None);
    assert_eq!(outcome.result, Ok(300));
    assert_eq!(outcome.seen.frames.len(), 300);
    assert!(
        queue
            .finish(|| MediaError::Parse("panicked".into()))
            .is_ok()
    );

    let mut queue = FrameQueue::spawn(
        FakeProducer {
            next: 0,
            end: 300,
            fail_at: Some(120),
        },
        4,
    );
    let outcome = run(0, 300, &mut || queue.recv(), working, None, None);
    let error = outcome.result.unwrap_err();
    assert!(error.0.contains("the decoder crashed"), "{error}");
    assert_eq!(outcome.seen.frames.len(), 120);
    assert!(!outcome.seen.finished);
}
