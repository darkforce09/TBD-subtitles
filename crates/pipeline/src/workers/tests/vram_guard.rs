use std::cell::RefCell;
use std::collections::VecDeque;

use super::*;

/// Free memory read from a list, the last value repeated once the list runs out; `None` stands
/// for a machine without NVML.
struct FakeMemory {
    readings: VecDeque<Option<u64>>,
    reads: usize,
}

impl FakeMemory {
    fn new(readings: &[Option<u64>]) -> FakeMemory {
        FakeMemory {
            readings: readings.iter().copied().collect(),
            reads: 0,
        }
    }
}

impl FreeMemory for FakeMemory {
    fn free_mib(&mut self) -> Option<u64> {
        self.reads += 1;
        if self.readings.len() > 1 {
            self.readings.pop_front().flatten()
        } else {
            self.readings.front().copied().flatten()
        }
    }
}

fn quick(deadline_ms: u64) -> Timing {
    Timing {
        poll: Duration::from_millis(5),
        deadline: Duration::from_millis(deadline_ms),
    }
}

/// Run a wait for 5,000 MiB on `memory`, and hand back its outcome and every line it said.
fn wait(
    memory: &mut FakeMemory,
    timing: Timing,
    cancel: &CancelToken,
) -> (Result<()>, Vec<String>) {
    let said = RefCell::new(Vec::new());
    let outcome = wait_with(
        StepName::TextInpaint,
        5_000,
        memory,
        timing,
        cancel,
        &|line| said.borrow_mut().push(line),
    );
    (outcome, said.into_inner())
}

#[test]
fn enough_free_memory_starts_at_once_and_says_nothing() {
    let mut memory = FakeMemory::new(&[Some(6_000)]);
    let (outcome, said) = wait(&mut memory, quick(1_000), &CancelToken::new());
    assert!(outcome.is_ok());
    assert!(said.is_empty());
    assert_eq!(memory.reads, 1);
}

#[test]
fn a_short_card_waits_saying_so_once_and_on_each_meaningful_change() {
    let mut memory = FakeMemory::new(&[
        Some(1_000),
        Some(1_050),
        Some(1_100),
        Some(3_000),
        Some(3_010),
        Some(5_000),
    ]);
    let (outcome, said) = wait(&mut memory, quick(10_000), &CancelToken::new());
    assert!(outcome.is_ok());
    assert_eq!(
        said,
        [
            "waiting for GPU memory: 1000 MiB free, 5000 needed",
            "waiting for GPU memory: 3000 MiB free, 5000 needed",
        ]
    );
    assert_eq!(memory.reads, 6);
}

#[test]
fn a_wait_past_the_deadline_fails_naming_the_memory_and_the_remedy() {
    let mut memory = FakeMemory::new(&[Some(1_000)]);
    let (outcome, said) = wait(&mut memory, quick(40), &CancelToken::new());
    let error = outcome.expect_err("deadline");
    assert!(!error.is_cancelled());
    assert_eq!(error.context, "step text_inpaint");
    assert!(error.message.contains("only 1000 MiB"), "{error}");
    assert!(error.message.contains("needs 5000 MiB"), "{error}");
    assert!(
        error.message.contains("close other GPU programs and retry"),
        "{error}"
    );
    assert_eq!(said.len(), 1);
}

#[test]
fn a_cancelled_wait_gives_up() {
    let mut memory = FakeMemory::new(&[Some(1_000)]);
    let cancel = CancelToken::new();
    let said = RefCell::new(0);
    let outcome = wait_with(
        StepName::Alignment,
        5_000,
        &mut memory,
        quick(60_000),
        &cancel,
        &|_| {
            *said.borrow_mut() += 1;
            cancel.cancel();
        },
    );
    assert!(outcome.expect_err("cancelled").is_cancelled());
    assert_eq!(said.into_inner(), 1);
}

#[test]
fn without_nvml_there_is_no_check() {
    let mut memory = FakeMemory::new(&[None]);
    let (outcome, said) = wait(&mut memory, quick(0), &CancelToken::new());
    assert!(outcome.is_ok());
    assert!(said.is_empty());
    let mut memory = FakeMemory::new(&[Some(1_000), None]);
    let (outcome, said) = wait(&mut memory, quick(10_000), &CancelToken::new());
    assert!(outcome.is_ok(), "NVML gone during the wait ends the check");
    assert_eq!(said.len(), 1);
}

#[test]
fn a_step_without_a_need_never_waits() {
    let outcome = wait_for_memory(StepName::Cues, &CancelToken::new(), &|_| {
        panic!("no line for a step without a need")
    });
    assert!(outcome.is_ok());
}

#[test]
fn the_deadline_is_ten_minutes_said_in_words() {
    assert_eq!(DEADLINE, Duration::from_secs(600));
    assert_eq!(waited_words(DEADLINE), "10 minutes");
    assert_eq!(waited_words(Duration::from_secs(60)), "1 minute");
    assert_eq!(waited_words(Duration::from_millis(40)), "0 s");
}
