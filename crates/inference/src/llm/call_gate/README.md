# Call gate

One cap on how many language-model calls run at once, shared by every
[Fix It](/documentation/glossary.md#fix-it) run that holds a seat, and a backend wrapper whose
calls pass it and are asked again when the provider is busy.

## Contents

```text
crates/inference/src/llm/call_gate/
├── gated.rs  `Gated`: a backend whose every call takes a slot, `Retry` and `is_transient`
├── mod.rs    `CallGate` (limit, slots held, the waiting line), `CallSeat` and `Permit`
└── tests/    unit tests for the cap, the order, cancelling, the limit and the retries
```

## How it works

`CallGate::new(limit)` makes the gate; each run takes one `CallSeat` with `gate.seat()`, and seats
are numbered in the order they are taken. `CallSeat::acquire` puts the call in line under its
seat's number and its arrival, and admits it once fewer calls than the limit hold a slot and it is
first in line; the `Permit` it returns frees the slot when dropped. So every call of a run started
earlier, even one of its later passes, goes before any call of a run started later. A waiter
looks at its cancel flag at least every 100 ms and leaves the line once it is set.
`set_limit` changes the cap at once: raising it admits waiters, lowering it stops no running call.
`CallSeat::waiting_only` says whether a run has calls waiting and none running.

`Gated::new(inner, seat, cancel)` wraps a backend: each `complete_json` takes a slot, calls
`inner` and frees the slot. A transient failure (`is_transient`: "rate limit", "rate_limit",
"overloaded", "too many requests", or 429 or 529 as a whole number) is asked again after each
delay of its `Retry` (30 s, 60 s and 120 s by default), without a slot during the pause and
logged as a `tracing` warning; once the delays run out the last error comes back with
" (after N tries)". A used-up subscription ("usage limit") and every other failure come back at
once.

## Boundaries

- Depends on: `std` (a mutex and a condition variable), `super::LanguageModel` and `tracing`.
- Used by: `crates/pipeline/src/fix_it/` (each `claude` backend wrapped in `Gated`, inside the
  answer cache) and `apps/tbd_subtitles/` (the gate a Fix It run's seat is taken from).
- Rules:
  - no more calls hold a slot than the limit (`no_more_calls_than_the_limit_hold_a_slot`);
  - a run started earlier goes first, a run's calls in arrival order
    (`a_run_started_earlier_goes_first`, `calls_of_one_run_go_in_arrival_order`);
  - a cancelled wait takes no slot and leaves the line (`a_cancelled_wait_ends_and_leaves_the_line`);
  - the slot is free during a retry pause (`the_slot_is_free_during_a_pause`), and a lasting
    failure is not asked again (`a_lasting_failure_is_not_asked_again`).

## Related documentation

- [Fix It](/documentation/features/fix_it.md#design) — why the calls of many runs share one cap.
