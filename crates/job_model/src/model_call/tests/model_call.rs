use super::*;

#[test]
fn a_call_goes_through_a_worker_line_unchanged() {
    let call = ModelExchange {
        id: "42-3".into(),
        model: "sonnet".into(),
        purpose: "words of the batch from U0012".into(),
        system: "You settle words.\nTwo lines.".into(),
        message: "U0012 hello".into(),
        schema: "{}".into(),
        answer: "{\"lines\": []}".into(),
        error: None,
        input_tokens: 10,
        output_tokens: 2,
        cost_usd: Some(0.01),
        seconds: 1.5,
    };
    let line = call.worker_line().unwrap();
    assert!(line.starts_with("model-call {"));
    assert!(!line.contains('\n'), "one line, whatever the prompt holds");
    assert_eq!(ModelExchange::from_worker_line(&line), Some(call));
}

#[test]
fn any_other_line_carries_no_call() {
    assert_eq!(ModelExchange::from_worker_line("progress 1 2"), None);
    assert_eq!(ModelExchange::from_worker_line("model-call {broken"), None);
}
