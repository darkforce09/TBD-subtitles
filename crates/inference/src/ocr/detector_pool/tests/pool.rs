use std::time::Duration;

use super::session::{CudaTuning, DetectorSession, Opened};
use super::*;
use crate::ocr::pool::{PaddedFrame, Priority};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 40;

/// A fake detector: text wherever the blue input is bright; a red first pixel makes the run
/// slow, so tests can make jobs finish out of order.
struct Fake {
    staging: Vec<f32>,
    maps: Vec<f32>,
    input: InputShape,
    role: Role,
    events: Arc<Mutex<Vec<String>>>,
}

impl DetectorSession for Fake {
    fn staging(&mut self) -> &mut [f32] {
        &mut self.staging
    }

    fn run(
        &mut self,
        read: &mut dyn FnMut(&[f32]) -> Result<(), OcrError>,
    ) -> Result<(), OcrError> {
        let plane = self.input.plane();
        if self.staging[2 * plane] > 0.0 {
            std::thread::sleep(Duration::from_millis(150));
        }
        for image in 0..self.input.batch {
            let blue = &self.staging[image * 3 * plane..image * 3 * plane + plane];
            let map = &mut self.maps[image * plane..(image + 1) * plane];
            for (probability, value) in map.iter_mut().zip(blue) {
                *probability = if *value > 0.0 { 0.9 } else { 0.0 };
            }
        }
        read(&self.maps)
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.events
            .lock()
            .unwrap()
            .push(format!("close {}", self.role.label()));
    }
}

#[derive(Default)]
struct FakeOpener {
    events: Arc<Mutex<Vec<String>>>,
    refuse_graph: bool,
    fail: Option<Role>,
}

impl SessionOpener for FakeOpener {
    fn open(&self, spec: &SessionSpec, tuning: CudaTuning) -> Result<Opened, OcrError> {
        if self.fail == Some(spec.role) || (self.refuse_graph && tuning.cuda_graph) {
            return Err(format!("no {} session on this card", spec.role.label()).into());
        }
        self.events
            .lock()
            .unwrap()
            .push(format!("open {}", spec.role.label()));
        Ok(Opened {
            session: Box::new(Fake {
                staging: vec![0.0; spec.input.len()],
                maps: vec![0.0; spec.input.output_len()],
                input: spec.input,
                role: spec.role,
                events: Arc::clone(&self.events),
            }),
            builds_engine: false,
            notes: Vec::new(),
        })
    }
}

fn options(sessions: usize, batch: usize) -> PoolOptions {
    PoolOptions {
        engine: DetectorEngine::Cuda,
        search: SearchMode::Fast,
        identity: EngineIdentity::default(),
        shape: ScreenShape {
            batch,
            pool_mib: 256,
        },
        sessions,
        cache_dir: PathBuf::from("tensorrt"),
        frame_width: WIDTH,
        frame_height: HEIGHT,
        tensorrt_fp16: true,
        vram_cap_mib: DEFAULT_VRAM_CAP_MIB,
    }
}

fn open(opener: FakeOpener, options: PoolOptions) -> Result<DetectorPool, OcrError> {
    DetectorPool::open_with(Arc::new(opener), Path::new("models"), options)
}

/// A frame with a white block over `[left, right)` × `[top, bottom)`, and a red first pixel
/// when `slow`.
fn frame(block: Option<[u32; 4]>, slow: bool) -> PaddedFrame {
    let padded_height = PaddedFrame::padded(HEIGHT);
    let mut rgb = vec![0u8; (WIDTH * padded_height * 3) as usize];
    if let Some([left, top, right, bottom]) = block {
        for y in top..bottom {
            for x in left..right {
                let at = ((y * WIDTH + x) * 3) as usize;
                rgb[at..at + 3].fill(255);
            }
        }
    }
    if slow {
        rgb[0] = 255;
    }
    PaddedFrame {
        width: WIDTH,
        height: HEIGHT,
        padded_height,
        rgb,
    }
}

fn job(seq: u64, priority: Priority, frames: Vec<PaddedFrame>) -> ScreenJob {
    ScreenJob {
        seq,
        priority,
        frames,
    }
}

#[test]
fn a_short_job_is_padded_and_answers_one_region_list_per_frame() {
    let mut pool = open(FakeOpener::default(), options(1, 3)).unwrap();
    let frames = vec![frame(Some([10, 10, 40, 20]), false), frame(None, false)];
    pool.submit(job(7, Priority::Screen, frames)).unwrap();
    let result = pool.recv().unwrap();
    assert_eq!(result.seq, 7);
    assert_eq!(result.regions.len(), 2);
    assert_eq!(result.regions[0].len(), 1);
    assert!(result.regions[1].is_empty());
    assert!(pool.recv().is_err(), "nothing is outstanding");
}

#[test]
fn regions_reaching_into_the_padding_are_clipped_to_the_frame() {
    let mut pool = open(FakeOpener::default(), options(1, 1)).unwrap();
    // The block runs from row 30 to the frame's last row; the padding below stays black.
    pool.submit(job(
        0,
        Priority::Screen,
        vec![frame(Some([8, 30, 48, HEIGHT]), false)],
    ))
    .unwrap();
    let regions = pool.recv().unwrap().regions;
    let (_, _, _, bottom) = regions[0][0].0.bounds();
    assert_eq!(bottom, f64::from(HEIGHT));
}

#[test]
fn results_finish_out_of_order_and_reorder_by_number() {
    let mut pool = open(FakeOpener::default(), options(2, 1)).unwrap();
    pool.submit(job(0, Priority::Screen, vec![frame(None, true)]))
        .unwrap();
    pool.submit(job(1, Priority::Screen, vec![frame(None, false)]))
        .unwrap();
    let first = pool.recv().unwrap();
    let second = pool.recv().unwrap();
    assert_eq!((first.seq, second.seq), (1, 0));
    let mut order = InOrder::new(0);
    order.push(first).unwrap();
    assert!(order.pop().is_none());
    order.push(second).unwrap();
    let released: Vec<u64> = std::iter::from_fn(|| order.pop()).map(|r| r.seq).collect();
    assert_eq!(released, [0, 1]);
}

#[test]
fn a_probe_overtakes_waiting_screening_jobs() {
    let mut pool = open(FakeOpener::default(), options(1, 1)).unwrap();
    pool.submit(job(0, Priority::Screen, vec![frame(None, true)]))
        .unwrap();
    for seq in [1, 2] {
        pool.submit(job(seq, Priority::Screen, vec![frame(None, false)]))
            .unwrap();
    }
    pool.submit(job(3, Priority::Probe, vec![frame(None, false)]))
        .unwrap();
    let order: Vec<u64> = (0..4).map(|_| pool.recv().unwrap().seq).collect();
    let at = |seq| order.iter().position(|&s| s == seq).unwrap();
    assert!(at(3) < at(1) && at(3) < at(2), "{order:?}");
    assert!(at(1) < at(2), "{order:?}");
}

#[test]
fn confirmations_answer_in_the_order_given_after_screening_closes() {
    let opener = FakeOpener::default();
    let events = Arc::clone(&opener.events);
    let mut pool = open(opener, options(2, 2)).unwrap();
    let lefts = [8, 24, 40, 16];
    let jobs = lefts
        .iter()
        .enumerate()
        .map(|(index, &left)| ConfirmJob {
            seq: 10 + index as u64,
            frame: frame(Some([left, 8, left + 20, 18]), index == 0),
        })
        .collect();
    let confirmed = pool.confirm(jobs).unwrap();
    let seqs: Vec<u64> = confirmed.iter().map(|c| c.seq).collect();
    assert_eq!(seqs, [10, 11, 12, 13]);
    for (result, left) in confirmed.iter().zip(lefts) {
        let (found_left, _, _, _) = result.regions[0].0.bounds();
        assert!(
            (found_left - f64::from(left)).abs() < 6.0,
            "{found_left} for {left}"
        );
    }
    let events = events.lock().unwrap().clone();
    let last_close = events.iter().rposition(|e| e == "close screen").unwrap();
    let first_confirm = events.iter().position(|e| e == "open confirm").unwrap();
    assert!(last_close < first_confirm, "{events:?}");
    assert_eq!(events.iter().filter(|e| *e == "close screen").count(), 2);
    assert!(events.iter().filter(|e| *e == "open confirm").count() <= 2);
    assert!(
        pool.submit(job(0, Priority::Screen, vec![frame(None, false)]))
            .is_err()
    );
}

#[test]
fn confirmation_waits_for_every_screening_result() {
    let mut pool = open(FakeOpener::default(), options(1, 1)).unwrap();
    pool.submit(job(0, Priority::Screen, vec![frame(None, false)]))
        .unwrap();
    assert!(pool.confirm(Vec::new()).is_err());
    pool.recv().unwrap();
    assert!(pool.confirm(Vec::new()).unwrap().is_empty());
}

#[test]
fn a_refused_cuda_graph_reopens_and_shows_in_the_notes() {
    let opener = FakeOpener {
        refuse_graph: true,
        ..FakeOpener::default()
    };
    let pool = open(opener, options(2, 1)).unwrap();
    let notes = pool.notes();
    for thread in [1, 2] {
        let key = format!("screen session options (thread {thread})");
        assert!(notes[&key].contains("no screen session"), "{notes:?}");
    }
    assert_eq!(notes["detector engine"], "CUDA");
    assert_eq!(notes["screen shape"], "1 × 3 × 64 × 64, pool 256 MiB");
    assert_eq!(notes["confirm shape"], "1 × 3 × 64 × 64, pool 1536 MiB");
    assert!(notes["search mode"].starts_with("fast"));
    assert!(pool.warmup_s() >= 0.0);
    assert_eq!(pool.engine_build_s(), 0.0);
}

#[test]
fn a_session_that_cannot_open_fails_the_pool() {
    let opener = FakeOpener {
        fail: Some(Role::Screen),
        ..FakeOpener::default()
    };
    let error = open(opener, options(2, 1)).err().unwrap().to_string();
    assert!(error.contains("no screen session"), "{error}");
}

#[test]
fn a_confirming_session_that_cannot_open_fails_the_confirmation() {
    let opener = FakeOpener {
        fail: Some(Role::Confirm),
        ..FakeOpener::default()
    };
    let mut pool = open(opener, options(2, 1)).unwrap();
    let jobs = (0..3)
        .map(|seq| ConfirmJob {
            seq,
            frame: frame(None, false),
        })
        .collect();
    let error = pool.confirm(jobs).err().unwrap().to_string();
    assert!(error.contains("no confirm session"), "{error}");
}

#[test]
fn tensorrt_without_an_identity_is_refused_before_any_session_opens() {
    let opener = FakeOpener::default();
    let events = Arc::clone(&opener.events);
    let mut tensorrt = options(1, 1);
    tensorrt.engine = DetectorEngine::TensorRt;
    assert!(open(opener, tensorrt).is_err());
    assert!(events.lock().unwrap().is_empty());
}

#[test]
fn frames_of_another_size_are_refused_at_submission() {
    let mut pool = open(FakeOpener::default(), options(1, 2)).unwrap();
    let mut wrong = frame(None, false);
    wrong.width = 32;
    assert!(pool.submit(job(0, Priority::Screen, vec![wrong])).is_err());
    let three = vec![frame(None, false), frame(None, false), frame(None, false)];
    assert!(pool.submit(job(1, Priority::Screen, three)).is_err());
}

#[test]
fn tensorrt_sessions_carry_their_cache_and_workspace() {
    let mut tensorrt = options(2, 4);
    tensorrt.engine = DetectorEngine::TensorRt;
    tensorrt.shape.pool_mib = 2_304;
    tensorrt.identity = EngineIdentity {
        gpu_name: "RTX 3070".to_owned(),
        driver: "580".to_owned(),
        tensorrt_version: "10.14".to_owned(),
    };
    let screen = tensorrt.spec(Role::Screen, Path::new("models"));
    let confirm = tensorrt.spec(Role::Confirm, Path::new("models"));
    assert_eq!(screen.model, Path::new("models/pp-ocrv5/det_mobile.onnx"));
    assert_eq!(confirm.model, Path::new("models/pp-ocrv5/det.onnx"));
    assert_eq!(screen.input.dims(), [4, 3, 64, 64]);
    assert_eq!(confirm.input.dims(), [1, 3, 64, 64]);
    let plan = screen.tensorrt.unwrap();
    assert_eq!(plan.cache_root, Path::new("tensorrt"));
    assert_eq!(plan.workspace_mib, 768);
    assert!(plan.fp16);
    assert!(
        options(1, 1)
            .spec(Role::Screen, Path::new("m"))
            .tensorrt
            .is_none()
    );
}
