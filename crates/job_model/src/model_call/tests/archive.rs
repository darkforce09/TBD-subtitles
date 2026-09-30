use super::ModelExchange;
use crate::archive_round_trip::round_trip;

#[test]
fn model_exchange_round_trips() {
    round_trip(&ModelExchange {
        id: "4242-7".into(),
        model: "sonnet".into(),
        purpose: "words of the batch from U0012".into(),
        system: "You settle what the engines heard.".into(),
        message: "U0012 P: I'm gonna be King of the Pirates!".into(),
        schema: "{\n  \"type\": \"object\"\n}".into(),
        answer: "{\n  \"lines\": []\n}".into(),
        error: Some("the call timed out".into()),
        input_tokens: 12_345,
        output_tokens: 678,
        cost_usd: Some(0.0425),
        seconds: 17.5,
    });
}
