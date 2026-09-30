use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::*;
use crate::job_queue::services::queue_editing::add_videos;

fn queue_of(states: Vec<JobState>) -> Queue {
    let mut queue = Queue::default();
    let videos: Vec<PathBuf> = (0..states.len())
        .map(|n| PathBuf::from(format!("{n}.mp4")))
        .collect();
    add_videos(&mut queue, videos);
    for (item, state) in queue.items.iter_mut().zip(states) {
        item.state = state;
    }
    queue
}

#[test]
fn this_process_runs_and_a_pid_that_cannot_exist_does_not() {
    assert!(process_runs(std::process::id()));
    assert!(!process_runs(u32::MAX));
}

#[test]
fn a_named_owner_is_gone_once_its_process_is_and_an_unknown_one_after_five_seconds() {
    let since = Instant::now();
    assert!(!owner_gone(Some(std::process::id()), since, since));
    assert!(owner_gone(Some(u32::MAX), since, since));
    assert!(!owner_gone(None, since, since + Duration::from_secs(4)));
    assert!(owner_gone(None, since, since + UNKNOWN_OWNER_WAIT));
}

#[test]
fn only_the_busy_jobs_whose_owner_is_gone_wait_again_in_place() {
    let now = Instant::now();
    let mut queue = queue_of(vec![
        JobState::Busy {
            owner: Some(std::process::id()),
            since: now,
        },
        JobState::Busy {
            owner: Some(u32::MAX),
            since: now,
        },
        JobState::Waiting,
        JobState::Busy {
            owner: None,
            since: now,
        },
    ]);
    assert!(any_busy(&queue));
    assert!(release_freed(&mut queue, now));
    let states: Vec<&JobState> = queue.items.iter().map(|item| &item.state).collect();
    assert!(states[0].is_busy());
    assert!(states[1].is_waiting());
    assert!(states[2].is_waiting());
    assert!(states[3].is_busy());

    assert!(release_freed(&mut queue, now + UNKNOWN_OWNER_WAIT));
    assert!(queue.items[3].state.is_waiting());
    assert!(!release_freed(&mut queue, now + UNKNOWN_OWNER_WAIT));
    assert!(queue.items[0].state.is_busy());
}
