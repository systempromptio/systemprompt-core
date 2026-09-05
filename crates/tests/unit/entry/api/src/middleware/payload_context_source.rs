//! Context extraction from the A2A JSON-RPC body.
//!
//! The context id decides which conversation a message joins, so guessing one
//! when the body does not carry it would silently splice a request into
//! somebody else's thread. Every path that cannot name a context has to refuse
//! and say which field was missing.

use axum::body::Body;
use axum::extract::Request;
use systemprompt_api::services::middleware::context::PayloadSource;
use systemprompt_models::execution::{ContextExtractionError, ContextIdSource};

#[test]
fn a_task_method_takes_its_context_from_the_task_id() {
    let body = br#"{"jsonrpc":"2.0","method":"tasks/get","params":{"id":"task-77"}}"#;

    let source =
        PayloadSource::extract_context_source(body).expect("a task id is a context source");

    match source {
        ContextIdSource::FromTask { task_id } => assert_eq!(task_id.as_str(), "task-77"),
        other => panic!("expected FromTask, got {other:?}"),
    }
}

#[test]
fn a_task_method_with_no_id_names_the_field_it_was_missing() {
    let body = br#"{"jsonrpc":"2.0","method":"tasks/get","params":{}}"#;

    let error = PayloadSource::extract_context_source(body)
        .err()
        .expect("a task method without a task id cannot resolve a context");

    match error {
        ContextExtractionError::InvalidHeaderValue { header, .. } => assert_eq!(
            header, "params.id",
            "the refusal must point at the field the caller has to fix"
        ),
        other => panic!("expected InvalidHeaderValue, got {other:?}"),
    }
}

#[test]
fn a_message_method_takes_the_context_id_the_message_carries() {
    let body =
        br#"{"jsonrpc":"2.0","method":"message/send","params":{"message":{"contextId":"ctx-9"}}}"#;

    let source = PayloadSource::extract_context_source(body).expect("an explicit contextId");

    match source {
        ContextIdSource::Direct(id) => assert_eq!(id, "ctx-9"),
        other => panic!("expected Direct, got {other:?}"),
    }
}

#[test]
fn a_message_with_no_context_id_is_a_missing_context_not_a_fabricated_one() {
    let body = br#"{"jsonrpc":"2.0","method":"message/send","params":{"message":{}}}"#;

    let error = PayloadSource::extract_context_source(body)
        .err()
        .expect("no contextId means no context");

    assert!(
        matches!(error, ContextExtractionError::MissingContextId),
        "got {error:?}"
    );
}

#[test]
fn a_body_that_is_not_json_is_reported_as_an_invalid_payload() {
    let error = PayloadSource::extract_context_source(b"<not json>")
        .err()
        .expect("a non-JSON body has no context to extract");

    match error {
        ContextExtractionError::InvalidHeaderValue { header, reason } => {
            assert_eq!(header, "payload");
            assert!(
                reason.starts_with("Invalid JSON:"),
                "the parse failure must be preserved; got {reason}"
            );
        },
        other => panic!("expected InvalidHeaderValue, got {other:?}"),
    }
}

// Why: the body is consumed to read it, so the middleware hands a rebuilt
// request downstream. A handler that received an empty body here would see an
// A2A request with no params at all.
#[tokio::test]
async fn reading_the_body_hands_back_a_request_that_still_carries_it() {
    let raw = br#"{"jsonrpc":"2.0","method":"message/send"}"#;
    let request = Request::builder()
        .uri("/a2a")
        .body(Body::from(raw.to_vec()))
        .expect("request builds");

    let (bytes, rebuilt) = PayloadSource::read_and_reconstruct(request)
        .await
        .expect("a well-formed body reads");

    assert_eq!(bytes, raw.to_vec());
    let replayed = axum::body::to_bytes(rebuilt.into_body(), usize::MAX)
        .await
        .expect("the rebuilt body reads");
    assert_eq!(
        replayed.to_vec(),
        raw.to_vec(),
        "the downstream handler must see the same bytes the extractor did"
    );
}
