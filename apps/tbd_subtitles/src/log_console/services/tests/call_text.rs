use std::time::Duration;

use job_model::model_call::ModelExchange;

use super::*;

fn kept(error: Option<&str>) -> KeptCall {
    KeptCall {
        seq: 0,
        elapsed: Duration::from_millis(83_456),
        video: Some("Dressrosa 12".into()),
        step: Some("fix_it".into()),
        call: ModelExchange {
            id: "7-3".into(),
            model: "opus".into(),
            purpose: "words fixes, call 3 of 8".into(),
            system: "RULES".into(),
            message: "MESSAGE".into(),
            schema: "SCHEMA".into(),
            answer: "ANSWER".into(),
            error: error.map(str::to_string),
            input_tokens: 1200,
            output_tokens: 80,
            cost_usd: Some(0.0874),
            seconds: 41.26,
        },
    }
}

#[test]
fn a_summary_gives_the_time_the_tokens_and_the_cost() {
    assert_eq!(summary(&kept(None)), "41.3 s · 1200 in, 80 out · $0.0874");
    assert_eq!(summary(&kept(Some("overloaded"))), "41.3 s · $0.0874");
}

#[test]
fn copy_all_writes_every_part_in_the_order_sent() {
    let text = call_text(&kept(None));
    assert!(
        text.starts_with("Model call 7-3 at 01:23.456\nModel: opus\n"),
        "{text}"
    );
    assert!(text.contains("Where: Dressrosa 12 · Fix It"), "{text}");
    let order = ["RULES", "MESSAGE", "SCHEMA", "ANSWER"].map(|part| text.find(part).unwrap());
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{text}");
    assert!(call_text(&kept(Some("overloaded"))).contains("Outcome: failed: overloaded"));
}
