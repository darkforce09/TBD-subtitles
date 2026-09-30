use super::*;

fn opened(ms: f64, counter: u64) -> Attempt {
    Attempt {
        outcome: OpenOutcome::Opened { ms },
        repair_calls: None,
        counter: Some(CounterRead::Value(counter)),
    }
}

fn refused(ms: f64) -> Attempt {
    Attempt {
        outcome: OpenOutcome::Failed {
            ms,
            display: "Database already open. Cannot acquire lock.".to_string(),
            debug: "DatabaseAlreadyOpen".to_string(),
        },
        repair_calls: None,
        counter: None,
    }
}

#[test]
fn an_attempt_renders_repair_and_counter() {
    let mut attempt = opened(2.0, 7);
    assert_eq!(attempt.render(), "opened in 2.000 ms; counter=7");
    attempt.repair_calls = Some(0);
    assert_eq!(attempt.render(), "opened in 2.000 ms; no repair; counter=7");
    attempt.repair_calls = Some(3);
    attempt.counter = Some(CounterRead::Missing);
    assert_eq!(
        attempt.render(),
        "opened in 2.000 ms; repair ran (3 callback calls); counter=missing"
    );
}

#[test]
fn a_refused_attempt_renders_the_error_only() {
    assert_eq!(
        refused(0.1).render(),
        "failed in 0.100 ms: Database already open. Cannot acquire lock. | DatabaseAlreadyOpen"
    );
}

#[test]
fn a_summary_counts_ranges_and_distinct_errors() {
    let attempts = [opened(3.0, 4), refused(0.5), opened(1.0, 9), refused(0.2)];
    assert_eq!(
        summarize(&attempts),
        "opened 2/4, failed 2/4; open took 0.200–3.000 ms; counters read 4–9; error ×2: \
         Database already open. Cannot acquire lock. | DatabaseAlreadyOpen"
    );
}

#[test]
fn a_summary_of_nothing_says_so() {
    assert_eq!(summarize(&[]), "opened 0/0, failed 0/0");
}
