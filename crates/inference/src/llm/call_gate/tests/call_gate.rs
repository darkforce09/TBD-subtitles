use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::*;

/// Poll `ready` every millisecond for at most a second.
fn wait_until(ready: impl Fn() -> bool) {
    let started = Instant::now();
    while !ready() {
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "the gate never got there"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// A thread that takes a slot on `seat`, records `label` in `admitted`, and frees the slot.
fn queue(
    seat: &CallSeat,
    label: &'static str,
    admitted: &Arc<Mutex<Vec<&'static str>>>,
) -> JoinHandle<()> {
    let (seat, admitted) = (seat.clone(), admitted.clone());
    std::thread::spawn(move || {
        let permit = seat.acquire(&AtomicBool::new(false)).unwrap();
        admitted.lock().unwrap().push(label);
        drop(permit);
    })
}

#[test]
fn no_more_calls_than_the_limit_hold_a_slot() {
    let gate = CallGate::new(3);
    let (running, peak) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let threads: Vec<_> = (0..10)
        .map(|_| {
            let (seat, running, peak) = (gate.seat(), running.clone(), peak.clone());
            std::thread::spawn(move || {
                let permit = seat.acquire(&AtomicBool::new(false)).unwrap();
                let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(5));
                running.fetch_sub(1, Ordering::SeqCst);
                drop(permit);
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(peak.load(Ordering::SeqCst), 3);
    assert_eq!((gate.held(), gate.waiting()), (0, 0));
}

#[test]
fn a_run_started_earlier_goes_first() {
    let gate = CallGate::new(1);
    let (first, second) = (gate.seat(), gate.seat());
    let blocker = gate.seat().acquire(&AtomicBool::new(false)).unwrap();
    let admitted = Arc::new(Mutex::new(Vec::new()));
    let later = queue(&second, "second", &admitted);
    wait_until(|| gate.waiting() == 1);
    let earlier = queue(&first, "first", &admitted);
    wait_until(|| gate.waiting() == 2);
    drop(blocker);
    later.join().unwrap();
    earlier.join().unwrap();
    assert_eq!(*admitted.lock().unwrap(), ["first", "second"]);
}

#[test]
fn calls_of_one_run_go_in_arrival_order() {
    let gate = CallGate::new(1);
    let seat = gate.seat();
    let blocker = seat.acquire(&AtomicBool::new(false)).unwrap();
    let admitted = Arc::new(Mutex::new(Vec::new()));
    let mut threads = Vec::new();
    for (n, label) in ["a", "b", "c"].into_iter().enumerate() {
        threads.push(queue(&seat, label, &admitted));
        wait_until(|| gate.waiting() == n + 1);
    }
    drop(blocker);
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(*admitted.lock().unwrap(), ["a", "b", "c"]);
}

#[test]
fn a_cancelled_wait_ends_and_leaves_the_line() {
    let gate = CallGate::new(1);
    let blocker = gate.seat().acquire(&AtomicBool::new(false)).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let seat = gate.seat();
    let waiter = {
        let (seat, cancel) = (seat.clone(), cancel.clone());
        std::thread::spawn(move || seat.acquire(&cancel).is_none())
    };
    wait_until(|| gate.waiting() == 1);
    cancel.store(true, Ordering::SeqCst);
    assert!(waiter.join().unwrap(), "the cancelled wait took a slot");
    assert_eq!((gate.held(), gate.waiting()), (1, 0));
    assert!(!seat.waiting_only());
    assert!(seat.acquire(&cancel).is_none(), "a set flag takes no slot");
    drop(blocker);
    let other = gate.seat().acquire(&AtomicBool::new(false));
    assert!(other.is_some(), "the next call gets the freed slot");
}

#[test]
fn raising_the_limit_admits_waiters_at_once() {
    let gate = CallGate::new(1);
    let blocker = gate.seat().acquire(&AtomicBool::new(false)).unwrap();
    let hold = Arc::new(AtomicBool::new(true));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let (seat, hold) = (gate.seat(), hold.clone());
            std::thread::spawn(move || {
                let permit = seat.acquire(&AtomicBool::new(false)).unwrap();
                while hold.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(1));
                }
                drop(permit);
            })
        })
        .collect();
    wait_until(|| gate.waiting() == 2);
    gate.set_limit(3);
    // Both waiters get in while the first call still holds its slot.
    wait_until(|| gate.held() == 3);
    assert_eq!(gate.limit(), 3);
    hold.store(false, Ordering::SeqCst);
    for thread in threads {
        thread.join().unwrap();
    }
    drop(blocker);
}

#[test]
fn lowering_the_limit_stops_no_running_call() {
    let gate = CallGate::new(3);
    let never = AtomicBool::new(false);
    let running: Vec<_> = (0..3)
        .map(|_| gate.seat().acquire(&never).unwrap())
        .collect();
    gate.set_limit(1);
    assert_eq!((gate.held(), gate.limit()), (3, 1));
    let late = gate.seat();
    let waiter = {
        let late = late.clone();
        std::thread::spawn(move || late.acquire(&AtomicBool::new(false)).is_some())
    };
    wait_until(|| gate.waiting() == 1);
    let mut running = running.into_iter();
    running.next();
    running.next();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(gate.waiting(), 1, "one call still holds the only slot");
    running.next();
    assert!(waiter.join().unwrap());
}

#[test]
fn a_limit_of_zero_counts_as_one() {
    let gate = CallGate::new(0);
    assert_eq!(gate.limit(), 1);
    let permit = gate.seat().acquire(&AtomicBool::new(false));
    assert!(permit.is_some());
    gate.set_limit(0);
    assert_eq!(gate.limit(), 1);
}

#[test]
fn waiting_only_holds_while_every_call_of_the_run_waits() {
    let gate = CallGate::new(1);
    let seat = gate.seat();
    assert!(!seat.waiting_only(), "a run with no call");
    let blocker = gate.seat().acquire(&AtomicBool::new(false)).unwrap();
    let hold = Arc::new(AtomicBool::new(true));
    let waiter = {
        let (seat, hold) = (seat.clone(), hold.clone());
        std::thread::spawn(move || {
            let permit = seat.acquire(&AtomicBool::new(false)).unwrap();
            while hold.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(1));
            }
            drop(permit);
        })
    };
    wait_until(|| gate.waiting() == 1);
    assert!(seat.waiting_only());
    drop(blocker);
    wait_until(|| gate.held() == 1 && gate.waiting() == 0);
    assert!(!seat.waiting_only(), "its call holds a slot");
    hold.store(false, Ordering::SeqCst);
    waiter.join().unwrap();
    assert!(!seat.waiting_only());
}
