use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

/// A session that counts its runs and fails them when told to.
struct Counting {
    staging: Vec<f32>,
    fail: bool,
}

impl DetectorSession for Counting {
    fn staging(&mut self) -> &mut [f32] {
        &mut self.staging
    }

    fn run(
        &mut self,
        read: &mut dyn FnMut(&[f32]) -> Result<(), OcrError>,
    ) -> Result<(), OcrError> {
        if self.fail {
            return Err("graph capture failed".into());
        }
        read(&[])
    }
}

/// Opens `Counting` sessions, refusing the CUDA graph at opening or at its first run.
#[derive(Default)]
struct Opener {
    refuse_at_open: bool,
    refuse_at_run: bool,
    refuse_always: bool,
    builds_engine: bool,
    tunings: Mutex<Vec<CudaTuning>>,
    opened: AtomicUsize,
}

impl SessionOpener for Opener {
    fn open(&self, spec: &SessionSpec, tuning: CudaTuning) -> Result<Opened, OcrError> {
        self.tunings.lock().unwrap().push(tuning);
        if self.refuse_always || (self.refuse_at_open && tuning.cuda_graph) {
            return Err("CUDA graph refused".into());
        }
        self.opened.fetch_add(1, Ordering::SeqCst);
        if self.builds_engine {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        Ok(Opened {
            session: Box::new(Counting {
                staging: vec![f32::NAN; spec.input.len()],
                fail: self.refuse_at_run && tuning.cuda_graph,
            }),
            builds_engine: self.builds_engine,
            notes: Vec::new(),
        })
    }
}

fn spec() -> SessionSpec {
    SessionSpec {
        role: Role::Screen,
        model: PathBuf::from("det_mobile.onnx"),
        input: InputShape::for_frames(2, 32, 32),
        pool_mib: 64,
        engine: DetectorEngine::Cuda,
        search: SearchMode::Fast,
        tensorrt: None,
    }
}

#[test]
fn a_session_opens_with_every_option_and_warms_up() {
    let opener = Opener::default();
    let warm = open_warm(&opener, &spec()).unwrap();
    assert_eq!(*opener.tunings.lock().unwrap(), [CudaTuning::FULL]);
    assert_eq!(
        warm.notes,
        [(
            "screen session options".to_owned(),
            "CUDA graph and NHWC on".to_owned()
        )]
    );
    assert_eq!(warm.engine_build_s, 0.0);
    assert!(warm.warmup_s >= 0.0);
}

#[test]
fn a_refused_cuda_graph_reopens_once_without_it_and_says_so() {
    let opener = Opener {
        refuse_at_open: true,
        ..Opener::default()
    };
    let warm = open_warm(&opener, &spec()).unwrap();
    assert_eq!(
        *opener.tunings.lock().unwrap(),
        [CudaTuning::FULL, CudaTuning::PLAIN]
    );
    let (key, value) = &warm.notes[0];
    assert_eq!(key, "screen session options");
    assert!(value.starts_with("CUDA graph and NHWC off"), "{value}");
    assert!(value.contains("CUDA graph refused"), "{value}");
}

#[test]
fn a_warm_up_that_fails_with_the_graph_also_reopens_without_it() {
    let opener = Opener {
        refuse_at_run: true,
        ..Opener::default()
    };
    let warm = open_warm(&opener, &spec()).unwrap();
    assert_eq!(opener.opened.load(Ordering::SeqCst), 2);
    assert!(warm.notes[0].1.contains("graph capture failed"));
}

#[test]
fn a_session_that_fails_both_ways_is_an_error_naming_both() {
    let opener = Opener {
        refuse_always: true,
        ..Opener::default()
    };
    let error = open_warm(&opener, &spec()).err().unwrap().to_string();
    assert_eq!(opener.tunings.lock().unwrap().len(), 2);
    assert!(error.contains("with the CUDA graph and NHWC"), "{error}");
    assert!(error.contains("without them"), "{error}");
}

#[test]
fn an_engine_build_is_timed_apart_from_the_warm_up() {
    let opener = Opener {
        builds_engine: true,
        ..Opener::default()
    };
    let warm = open_warm(&opener, &spec()).unwrap();
    assert_eq!(warm.warmup_s, 0.0);
    assert!(warm.engine_build_s >= 0.005);
}
