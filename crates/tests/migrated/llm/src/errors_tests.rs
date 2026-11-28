//! Failures from the model endpoint: classification by status and body, the
//! readable message, retry advice and what is never re-sent.

use std::time::Duration;

use systemprompt_engine::llm::retry::{Retry, classify};
use systemprompt_engine::llm::{LlmError, Protocol, TransportError, is_context_overflow};

use crate::support::{Canned, collect, status_error, status_error_with};

fn excerpt(error: &LlmError) -> &str {
    match error {
        LlmError::Status { body_excerpt, .. } => body_excerpt,
        LlmError::ContextOverflow { message, .. } => message,
        other => panic!("expected a status or overflow error, got {other:?}"),
    }
}

#[tokio::test]
async fn token_limit_messages_are_a_context_overflow() {
    for body in [
        r#"{"error": {"type": "invalid_request_error", "message": "prompt is too long: 250000 tokens > 200000 maximum"}}"#,
        r#"{"error": {"code": "context_length_exceeded", "message": "This model's maximum context length is 128000 tokens"}}"#,
        r#"{"error": {"type": "request_too_large", "message": "Request exceeds the maximum size"}}"#,
    ] {
        let error = status_error(400, body).await;
        assert!(
            matches!(
                error,
                LlmError::ContextOverflow {
                    status: Some(400),
                    ..
                }
            ),
            "{body}: {error:?}"
        );
        assert_eq!(classify(&error), Retry::No);
    }
}

#[tokio::test]
async fn an_ordinary_invalid_request_is_not_an_overflow() {
    let error = status_error(400, r#"{"error": {"message": "messages: field required"}}"#).await;
    assert!(matches!(error, LlmError::Status { status: 400, .. }));
    assert_eq!(classify(&error), Retry::No);
}

#[tokio::test]
#[ignore = "parity ledger: a bare 413 payload error is treated as a context overflow rather than as an oversized request"]
async fn a_generic_413_is_an_oversized_request_not_an_overflow() {
    let error = status_error(413, "Payload Too Large").await;
    assert!(matches!(error, LlmError::Status { status: 413, .. }));
}

#[tokio::test]
async fn a_rate_limit_is_never_an_overflow_and_is_retried() {
    let error = status_error(
        429,
        r#"{"error": {"type": "rate_limit_error", "message": "too many tokens per minute"}}"#,
    )
    .await;
    assert!(matches!(error, LlmError::Status { status: 429, .. }));
    assert_eq!(classify(&error), Retry::Backoff);
    assert!(!is_context_overflow(
        None,
        "rate_limit_error: too many tokens"
    ));
}

#[tokio::test]
#[ignore = "parity ledger: a rate limit reported behind HTTP 400 is not retried"]
async fn a_rate_limit_behind_a_400_is_retried() {
    let error = status_error(
        400,
        r#"{"error": {"type": "rate_limit_error", "message": "Rate limit reached"}}"#,
    )
    .await;
    assert_ne!(classify(&error), Retry::No);
}

#[tokio::test]
async fn a_server_code_in_the_body_does_not_make_a_4xx_retryable() {
    let error = status_error(
        400,
        r#"{"error": {"type": "overloaded_error", "message": "server_error"}}"#,
    )
    .await;
    assert_eq!(classify(&error), Retry::No);
}

#[tokio::test]
async fn transient_statuses_are_retried_and_other_4xx_are_not() {
    for status in [408, 409, 425, 429, 500, 502, 503, 504, 529] {
        assert_eq!(
            classify(&status_error(status, "busy").await),
            Retry::Backoff,
            "{status}"
        );
    }
    for status in [400, 401, 402, 403, 404, 422] {
        assert_eq!(
            classify(&status_error(status, "no").await),
            Retry::No,
            "{status}"
        );
    }
}

#[tokio::test]
async fn the_server_wait_is_honoured_when_given() {
    let error = status_error_with(Canned::body(429, "slow down").header("retry-after", "7")).await;
    assert!(
        matches!(error, LlmError::Status { retry_after: Some(wait), .. } if wait == Duration::from_secs(7))
    );
    assert_eq!(classify(&error), Retry::After(Duration::from_secs(7)));
    let error = status_error_with(Canned::body(529, "{}").header("retry-after-ms", "250")).await;
    assert_eq!(classify(&error), Retry::After(Duration::from_millis(250)));
}

#[tokio::test]
async fn an_unrecognised_body_is_shown_as_it_came() {
    let error = status_error(500, "upstream exploded: code 7").await;
    assert_eq!(excerpt(&error), "upstream exploded: code 7");
}

#[tokio::test]
async fn messages_are_read_from_common_error_layouts() {
    for (body, expected) in [
        (
            r#"{"error": {"message": "nested message"}}"#,
            "nested message",
        ),
        (r#"{"message": "top-level message"}"#, "top-level message"),
        (r#"{"error": "plain error string"}"#, "plain error string"),
        (
            r#"{"message": "outer", "error": {"message": "inner wins"}}"#,
            "inner wins",
        ),
    ] {
        assert_eq!(excerpt(&status_error(400, body).await), expected, "{body}");
    }
}

#[tokio::test]
#[ignore = "parity ledger: an HTML error page is shown in the message instead of being withheld"]
async fn an_html_error_page_is_not_shown() {
    let error = status_error(
        502,
        "<!DOCTYPE html><html><body><h1>502 Bad Gateway</h1></body></html>",
    )
    .await;
    assert!(!excerpt(&error).contains("<html"));
}

#[tokio::test]
#[ignore = "parity ledger: a structured message in a body larger than the read limit is lost; only a raw excerpt is kept"]
async fn a_structured_message_survives_a_large_body() {
    let body = format!(
        r#"{{"error": {{"message": "the real reason"}}, "debug": "{}"}}"#,
        "x".repeat(40_000)
    );
    assert_eq!(excerpt(&status_error(400, &body).await), "the real reason");
}

#[tokio::test]
#[ignore = "parity ledger: an empty structured message yields an empty description instead of the body"]
async fn an_empty_structured_message_falls_back_to_the_body() {
    let error = status_error(
        400,
        r#"{"error": {"message": "", "type": "invalid_request_error"}}"#,
    )
    .await;
    assert!(excerpt(&error).contains("invalid_request_error"));
}

#[tokio::test]
async fn a_malformed_body_still_reports_the_status() {
    let error = status_error(503, "{{{ not json").await;
    assert!(matches!(error, LlmError::Status { status: 503, .. }));
    assert_eq!(excerpt(&error), "{{{ not json");
}

#[tokio::test]
async fn an_empty_body_still_reports_the_status() {
    let error = status_error(500, "").await;
    assert!(
        matches!(error, LlmError::Status { status: 500, ref body_excerpt, .. } if body_excerpt.is_empty())
    );
}

#[tokio::test]
async fn a_body_that_fails_to_arrive_still_reports_the_status() {
    let error = status_error_with(Canned::body(503, "").failing()).await;
    assert!(matches!(error, LlmError::Status { status: 503, .. }));
}

#[tokio::test]
async fn an_overflow_buried_in_a_vague_body_is_still_recognised() {
    let error = status_error(
        400,
        r#"{"error": {"message": "Bad request", "details": [{"reason": "prompt is too long"}]}}"#,
    )
    .await;
    assert!(matches!(error, LlmError::ContextOverflow { .. }));
}

#[tokio::test]
async fn a_refused_call_is_sent_once_and_never_re_sent_by_the_client() {
    let canned = Canned::body(500, "boom");
    let outcome = collect(&canned, Protocol::Messages).await;
    assert!(outcome.is_err());
    assert_eq!(canned.sent(), 1);
}

#[tokio::test]
async fn a_stream_that_fails_after_opening_is_not_re_sent() {
    let canned = Canned::body(
        200,
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{}}\n\n",
    )
    .failing();
    let items = collect(&canned, Protocol::Messages).await.unwrap();
    assert!(items.iter().any(Result::is_err));
    assert_eq!(canned.sent(), 1);
}

#[test]
fn a_dropped_connection_is_a_transport_error_that_is_retried() {
    let error = LlmError::Transport(TransportError::Io(std::io::Error::new(
        std::io::ErrorKind::ConnectionReset,
        "reset",
    )));
    assert_eq!(classify(&error), Retry::Backoff);
    assert!(std::error::Error::source(&error).is_some());
    assert_eq!(
        classify(&LlmError::Transport(TransportError::Timeout)),
        Retry::Backoff
    );
}

#[test]
fn errors_that_resending_cannot_fix_are_not_retried() {
    let decode = LlmError::Decode(serde_json::from_str::<serde_json::Value>("{").unwrap_err());
    assert_eq!(classify(&decode), Retry::No);
    assert_eq!(classify(&LlmError::Aborted), Retry::No);
    assert_eq!(classify(&LlmError::InvalidEndpoint), Retry::No);
}

#[test]
fn stream_errors_are_retried_only_when_they_read_as_transient() {
    let provider = |kind: &str, message: &str| LlmError::Provider {
        kind: Some(kind.into()),
        message: message.into(),
    };
    assert_eq!(
        classify(&provider("overloaded_error", "Overloaded")),
        Retry::Backoff
    );
    assert_eq!(
        classify(&provider("api_error", "Internal server error")),
        Retry::Backoff
    );
    assert_eq!(
        classify(&provider("server_error", "please try again")),
        Retry::Backoff
    );
    assert_eq!(
        classify(&provider("invalid_request_error", "bad tool schema")),
        Retry::No
    );
    assert_eq!(
        classify(&provider("weird_unknown_code", "something")),
        Retry::No
    );
}

#[test]
fn a_specific_failure_wins_over_retryable_wording() {
    assert!(is_context_overflow(
        None,
        "prompt is too long; please try again later"
    ));
    assert!(!is_context_overflow(Some(429), "prompt is too long"));
}
