use super::*;

#[test]
fn reads_the_structured_output_tokens_and_cost() {
    let stdout = r#"{"is_error":false,"total_cost_usd":0.0054,
      "usage":{"input_tokens":2,"cache_creation_input_tokens":1187,"cache_read_input_tokens":10,"output_tokens":64},
      "structured_output":{"t":"hello"}}"#;
    let c = parse(stdout).unwrap();
    assert_eq!(c.json["t"], "hello");
    assert_eq!((c.input_tokens, c.output_tokens), (1199, 64));
    assert_eq!(c.cost_usd, Some(0.0054));
}

#[test]
fn a_result_without_structured_output_is_an_error() {
    assert!(parse(r#"{"is_error":false,"result":"text"}"#).is_err());
    assert!(parse(r#"{"is_error":true,"result":"limit reached"}"#).is_err());
    assert!(parse("not json").is_err());
}
