use harness_llm::is_context_overflow;

#[test]
fn messages_overflow_is_detected() {
    let body = r#"{"type":"error","error":{"type":"invalid_request_error","message":"prompt is too long: 210000 tokens > 200000 maximum"}}"#;
    assert!(is_context_overflow(Some(400), body));
}

#[test]
fn chat_overflow_is_detected() {
    let body = r#"{"error":{"message":"This model's maximum context length is 128000 tokens.","code":"context_length_exceeded"}}"#;
    assert!(is_context_overflow(Some(400), body));
    assert!(is_context_overflow(Some(413), ""));
}

#[test]
fn rate_limits_are_not_overflow() {
    assert!(!is_context_overflow(
        Some(429),
        "too many tokens per minute"
    ));
    assert!(!is_context_overflow(
        Some(400),
        "rate limit: too many tokens per minute"
    ));
    assert!(!is_context_overflow(Some(400), "invalid tool schema"));
}
