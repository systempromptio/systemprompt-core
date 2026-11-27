use systemprompt_engine::llm::{LlmError, Protocol};

use crate::llm::support::{Canned, decode, open};

#[tokio::test]
async fn status_excerpt_is_bounded() {
    let long = "x".repeat(2_000);
    let result = open(&Canned::new(400, &[long.as_str()]), Protocol::Messages).await;
    let Err(LlmError::Status {
        status,
        body_excerpt,
        ..
    }) = result
    else {
        panic!("expected a status error");
    };
    assert_eq!(status, 400);
    assert_eq!(body_excerpt.chars().count(), 513);
    assert!(body_excerpt.ends_with('…'));
}

#[tokio::test]
async fn status_excerpt_is_trimmed() {
    let result = open(&Canned::new(500, &["  short  "]), Protocol::ChatCompletions).await;
    assert!(matches!(
        result,
        Err(LlmError::Status { status: 500, ref body_excerpt, .. }) if body_excerpt == "short"
    ));
}

#[tokio::test]
async fn in_stream_overflow_is_typed() {
    let body = concat!(
        "event: error\n",
        "data: {\"type\":\"error\",\"error\":{\"type\":\"invalid_request_error\",",
        "\"message\":\"prompt is too long: 250000 tokens\"}}\n\n",
    );
    let result = decode(Protocol::Messages, body, 7).await;
    assert!(matches!(
        result,
        Err(LlmError::ContextOverflow { status: None, .. })
    ));
}

#[tokio::test]
async fn in_stream_provider_error_keeps_its_kind() {
    let body = concat!(
        "event: error\n",
        "data: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",",
        "\"message\":\"Overloaded\"}}\n\n",
    );
    let result = decode(Protocol::Messages, body, 5).await;
    assert!(matches!(
        result,
        Err(LlmError::Provider { kind: Some(ref kind), .. }) if kind == "overloaded_error"
    ));
}
