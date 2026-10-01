use std::collections::{BTreeMap, VecDeque};

use inference::ocr::OcrError;
use inference::ocr::pool::{ConfirmJob, ConfirmResult, ScreenResult, ScreenShape};

use super::*;
use crate::onscreen_text::detect::fixtures::{Answer, Pool, SIZE};

fn frame() -> PaddedFrame {
    PaddedFrame {
        width: SIZE.0,
        height: SIZE.1,
        padded_height: PaddedFrame::padded(SIZE.1),
        rgb: vec![0; SIZE.0 as usize * PaddedFrame::padded(SIZE.1) as usize * 3],
    }
}

/// Sessions that answer with whatever results the test scripts.
struct Scripted {
    results: VecDeque<ScreenResult>,
}

impl TextScreening for Scripted {
    fn shape(&self) -> ScreenShape {
        ScreenShape::INITIAL
    }

    fn sessions(&self) -> usize {
        1
    }

    fn submit(&mut self, _job: ScreenJob) -> Result<(), OcrError> {
        Ok(())
    }

    fn recv(&mut self) -> Result<ScreenResult, OcrError> {
        self.results
            .pop_front()
            .ok_or_else(|| "nothing scripted".into())
    }

    fn confirm(&mut self, _jobs: Vec<ConfirmJob>) -> Result<Vec<ConfirmResult>, OcrError> {
        Ok(Vec::new())
    }

    fn warmup_s(&self) -> f64 {
        0.0
    }

    fn engine_build_s(&self) -> f64 {
        0.0
    }

    fn notes(&self) -> BTreeMap<String, String> {
        BTreeMap::new()
    }
}

#[test]
fn results_arriving_in_any_order_are_handed_out_by_number() {
    let mut pool = Pool::new(Vec::new(), 2, Answer::Newest);
    let mut flight = Flight::new(&mut pool);
    let first = flight.submit(Priority::Screen, vec![frame()]).unwrap();
    let second = flight
        .submit(Priority::Screen, vec![frame(), frame()])
        .unwrap();
    let third = flight.submit(Priority::Screen, vec![frame()]).unwrap();
    assert_eq!((first, second, third), (0, 1, 2));
    assert_eq!(flight.wait(first).unwrap().len(), 1);
    assert!(
        flight.answered(second) && flight.answered(third),
        "kept while waiting"
    );
    assert_eq!(flight.take(second).unwrap().len(), 2);
    assert_eq!(flight.take(third).unwrap().len(), 1);
    assert!(flight.take(third).is_none(), "a result is taken once");
    assert!(flight.receive().is_err(), "nothing is in flight");
    let answered: Vec<u64> = pool.answered.iter().map(|answered| answered.seq).collect();
    assert_eq!(answered, [2, 1, 0]);
}

#[test]
fn an_empty_job_is_never_submitted() {
    let mut pool = Pool::new(Vec::new(), 1, Answer::Oldest);
    let mut flight = Flight::new(&mut pool);
    assert!(flight.submit(Priority::Probe, Vec::new()).is_err());
    assert!(pool.queue.is_empty());
}

#[test]
fn a_result_for_no_job_in_flight_or_with_the_wrong_frame_count_is_an_error() {
    let unknown = ScreenResult {
        seq: 7,
        regions: vec![Vec::new()],
    };
    let mut pool = Scripted {
        results: VecDeque::from([unknown]),
    };
    let mut flight = Flight::new(&mut pool);
    flight.submit(Priority::Screen, vec![frame()]).unwrap();
    assert!(flight.receive().is_err());
    let short = ScreenResult {
        seq: 0,
        regions: Vec::new(),
    };
    let mut pool = Scripted {
        results: VecDeque::from([short]),
    };
    let mut flight = Flight::new(&mut pool);
    flight.submit(Priority::Screen, vec![frame()]).unwrap();
    assert!(flight.wait(0).is_err());
}
