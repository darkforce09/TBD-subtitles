use super::*;
use crate::ocr::pool::PaddedFrame;

fn screen(seq: u64, priority: Priority) -> ScreenJob {
    ScreenJob {
        seq,
        priority,
        frames: Vec::new(),
    }
}

fn confirm(seq: u64) -> ConfirmJob {
    ConfirmJob {
        seq,
        frame: PaddedFrame {
            width: 0,
            height: 0,
            padded_height: 0,
            rgb: Vec::new(),
        },
    }
}

fn order(state: &mut State) -> Vec<u64> {
    std::iter::from_fn(|| match state.take(true) {
        Some(Task::Screen(job)) => Some(job.seq),
        _ => None,
    })
    .collect()
}

#[test]
fn probes_run_first_then_screening_jobs_by_number() {
    let mut state = State::new(1);
    for seq in [3, 1, 2] {
        state.push_screen(screen(seq, Priority::Screen));
    }
    state.push_screen(screen(7, Priority::Probe));
    state.push_screen(screen(5, Priority::Probe));
    assert_eq!(order(&mut state), [5, 7, 1, 2, 3]);
}

#[test]
fn an_empty_queue_makes_the_thread_wait() {
    let mut state = State::new(2);
    assert!(state.take(true).is_none());
}

#[test]
fn confirmations_wait_until_every_screening_session_is_closed() {
    let mut state = State::new(2);
    state.push_confirm([(0, confirm(10)), (1, confirm(11))]);
    assert!(matches!(state.take(true), Some(Task::CloseScreen)));
    state.screen_closed();
    // One session is still open somewhere: nothing to confirm yet.
    assert!(state.take(false).is_none());
    assert!(matches!(state.take(true), Some(Task::CloseScreen)));
    state.screen_closed();
    assert!(matches!(state.take(false), Some(Task::Confirm(0, job)) if job.seq == 10));
    assert!(matches!(state.take(false), Some(Task::Confirm(1, job)) if job.seq == 11));
    assert!(state.take(false).is_none());
}

#[test]
fn closing_ends_every_thread() {
    let mut state = State::new(1);
    state.push_screen(screen(1, Priority::Screen));
    state.close();
    assert!(matches!(state.take(true), Some(Task::Exit)));
    assert!(matches!(state.take(false), Some(Task::Exit)));
}
