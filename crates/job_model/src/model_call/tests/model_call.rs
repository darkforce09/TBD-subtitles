use super::*;

#[test]
fn a_call_goes_through_its_json_unchanged() {
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
    let json = serde_json::to_vec(&call).unwrap();
    assert_eq!(
        serde_json::from_slice::<ModelExchange>(&json).unwrap(),
        call
    );
}

#[test]
fn json_that_is_not_a_whole_call_is_no_call() {
    assert!(serde_json::from_slice::<ModelExchange>(b"{broken").is_err());
    assert!(serde_json::from_slice::<ModelExchange>(b"[1, 2]").is_err());
}
