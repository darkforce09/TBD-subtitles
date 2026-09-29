use serde_json::json;

use super::*;

fn sent<'a>(schema: &'a serde_json::Value) -> Sent<'a> {
    Sent {
        model: "sonnet",
        system: "You settle words.",
        message: "U0012 hello\nU0013 there",
        schema,
    }
}

#[test]
fn a_purpose_lasts_as_long_as_its_guard_and_an_inner_one_hides_the_outer() {
    assert_eq!(current_purpose(), "");
    let outer = purpose("words of the batch from U0012");
    assert_eq!(current_purpose(), "words of the batch from U0012");
    {
        let _inner = purpose("re-asking U0013");
        assert_eq!(current_purpose(), "re-asking U0013");
    }
    assert_eq!(current_purpose(), "words of the batch from U0012");
    drop(outer);
    assert_eq!(current_purpose(), "");
}

#[test]
fn a_purpose_belongs_to_its_thread() {
    let _guard = purpose("on the main thread");
    let other = std::thread::spawn(current_purpose).join().unwrap();
    assert_eq!(other, "");
}

#[test]
fn an_answered_call_keeps_the_prompt_the_pretty_answer_and_its_cost() {
    let schema = json!({"type": "object"});
    let answer = Ok(Completion {
        json: json!({"lines": [1]}),
        input_tokens: 120,
        output_tokens: 30,
        cost_usd: Some(0.02),
    });
    let call = exchange(
        "7-1",
        "words",
        &sent(&schema),
        Duration::from_millis(1500),
        &answer,
        "{…}",
    );
    assert_eq!(call.id, "7-1");
    assert_eq!(call.purpose, "words");
    assert_eq!(call.system, "You settle words.");
    assert_eq!(call.message, "U0012 hello\nU0013 there");
    assert!(
        call.schema.contains("\"type\": \"object\""),
        "{}",
        call.schema
    );
    assert!(call.answer.contains("\"lines\": ["), "{}", call.answer);
    assert_eq!(call.error, None);
    assert_eq!((call.input_tokens, call.output_tokens), (120, 30));
    assert_eq!(call.cost_usd, Some(0.02));
    assert!((call.seconds - 1.5).abs() < 1e-9);
}

#[test]
fn a_failed_call_keeps_what_was_printed_and_why() {
    let schema = json!({});
    let answer = Err(LlmError("claude exited 1: overloaded".into()));
    let call = exchange(
        "7-2",
        "",
        &sent(&schema),
        Duration::from_secs(3),
        &answer,
        "{\"is_error\": true}",
    );
    assert_eq!(call.answer, "{\"is_error\": true}");
    assert_eq!(call.error.as_deref(), Some("claude exited 1: overloaded"));
    assert_eq!(call.cost_usd, None);
}

#[test]
fn image_payloads_are_replaced_by_their_size_in_the_exchange() {
    let line = serde_json::json!({
        "type": "user",
        "message": {"role": "user", "content": [
            {"type": "text", "text": "read this"},
            {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "QUJDRA=="}}
        ]},
        "parent_tool_use_id": null
    })
    .to_string();
    let elided = without_image_data(&line);
    assert!(!elided.contains("QUJDRA=="));
    assert!(elided.contains("[8 base64 bytes]"));
    assert!(elided.contains("read this"));
    assert_eq!(without_image_data("U0012 hello"), "U0012 hello");
    let schema = serde_json::json!({"type": "object"});
    let sent = Sent {
        model: "sonnet",
        system: "s",
        message: &line,
        schema: &schema,
    };
    let call = exchange(
        "1-1",
        "",
        &sent,
        Duration::from_secs(1),
        &Err(LlmError("x".into())),
        "",
    );
    assert!(call.message.contains("[8 base64 bytes]"));
}
