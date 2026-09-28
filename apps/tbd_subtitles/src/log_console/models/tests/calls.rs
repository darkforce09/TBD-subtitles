use std::time::Duration;

use job_model::model_call::ModelExchange;

use super::*;

fn kept(seq: u64, id: &str, purpose: &str, failed: bool) -> KeptCall {
    KeptCall {
        seq,
        elapsed: Duration::from_secs(seq),
        video: Some("Dressrosa 12".into()),
        step: Some(if failed { "fix_it" } else { "adjudicate" }.into()),
        call: ModelExchange {
            id: id.into(),
            model: "sonnet".into(),
            purpose: purpose.into(),
            message: "U0012 a long prompt".into(),
            error: failed.then(|| "overloaded".to_string()),
            ..ModelExchange::default()
        },
    }
}

fn sample() -> Calls {
    let mut calls = Calls::default();
    calls.append(vec![
        kept(0, "1-1", "words, batch 1 of 2", false),
        kept(1, "1-2", "words, batch 2 of 2", false),
        kept(2, "1-3", "words fixes, call 1 of 1", true),
    ]);
    calls
}

#[test]
fn calls_arrive_in_order_and_failures_are_counted() {
    let calls = sample();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls.shown_len(), 3);
    assert_eq!(calls.next(), 3);
    assert_eq!(calls.failed(), 1);
    assert_eq!(calls.shown_call(2).unwrap().call.id, "1-3");
}

#[test]
fn a_search_matches_the_purpose_or_the_step_but_never_the_prompt() {
    let mut calls = sample();
    calls.set_search("batch 2".into());
    assert_eq!(calls.shown_len(), 1);
    calls.set_search("fix_it".into());
    assert_eq!(calls.shown_call(0).unwrap().call.id, "1-3");
    calls.set_search("long prompt".into());
    assert_eq!(calls.shown_len(), 0);
}

#[test]
fn the_open_call_stays_open_under_a_search_until_cleared() {
    let mut calls = sample();
    calls.select(Some("1-2".into()));
    calls.set_search("fix".into());
    assert_eq!(
        calls.selected().unwrap().call.purpose,
        "words, batch 2 of 2"
    );
    calls.clear();
    assert!(calls.selected().is_none());
    assert_eq!(calls.next(), 3);
}

#[test]
fn past_capacity_the_oldest_calls_leave() {
    let mut calls = Calls::default();
    let many = (0..=CALL_CAPACITY as u64)
        .map(|seq| kept(seq, &seq.to_string(), "", false))
        .collect();
    calls.append(many);
    assert_eq!(calls.len(), CALL_CAPACITY);
    assert_eq!(calls.shown_call(0).unwrap().call.id, "1");
}
