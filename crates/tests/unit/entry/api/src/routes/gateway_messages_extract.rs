//! Header, body, and conversation extraction for the gateway `/messages`
//! handler.
//!
//! Everything here runs before authentication, so it is the gateway's first
//! line of input validation: a session header is mandatory, a conversation
//! header is optional but must be well-formed when present, the body must
//! parse into the canonical model, and a conversation id must be derivable
//! when the caller did not supply one. Each rejection also records what it
//! learned into the `RejectionPartial` that the audit row is built from, so
//! the tests assert the partial as well as the status.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use std::sync::Arc;
use systemprompt_api::routes::gateway::messages::dispatch::errors::build_error_response;
use systemprompt_api::routes::gateway::messages::extract::headers::{
    optional_gateway_conversation_id, read_gateway_body, require_session_id,
};
use systemprompt_api::routes::gateway::messages::extract::{RejectionPartial, derive_conversation};
use systemprompt_api::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, Role,
};
use systemprompt_api::services::gateway::protocol::inbound::InboundAdapter;
use systemprompt_api::services::gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_identifiers::headers::{GATEWAY_CONVERSATION_ID, SESSION_ID};
use systemprompt_identifiers::{ClientSessionId, ContextId, GatewayConversationId, SessionId};

fn headers_with(name: &'static str, value: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        name,
        HeaderValue::from_str(value).expect("test header value must be valid"),
    );
    headers
}

fn inbound() -> Arc<dyn InboundAdapter> {
    Arc::new(AnthropicMessagesInbound)
}

fn post(body: &'static str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .body(Body::from(body))
        .expect("test request must build")
}

fn canonical(messages: Vec<CanonicalMessage>) -> CanonicalRequest {
    CanonicalRequest {
        model: "claude-test".into(),
        system: None,
        messages,
        max_tokens: 16,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: vec![],
        tools: vec![],
        tool_choice: None,
        stream: false,
        thinking: None,
        metadata: None,
        response_format: None,
        reasoning_effort: None,
        search: None,
        code_execution: false,
        presence_penalty: None,
        frequency_penalty: None,
        forwarded_surface: Default::default(),
    }
}

fn user_message(text: &str) -> CanonicalMessage {
    CanonicalMessage {
        role: Role::User,
        content: vec![CanonicalContent::Text(text.into())],
    }
}

#[test]
fn a_missing_session_header_is_a_400_naming_the_header() {
    let (status, message) =
        require_session_id(&HeaderMap::new()).expect_err("the session header is mandatory");

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message.contains(SESSION_ID), "{message}");
    assert!(message.contains("missing required"), "{message}");
}

#[test]
fn a_blank_session_header_is_rejected_rather_than_treated_as_present() {
    let (status, message) = require_session_id(&headers_with(SESSION_ID, "   "))
        .expect_err("whitespace is not a session id");

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message.contains("empty"), "{message}");
}

#[test]
fn a_session_header_is_trimmed_before_it_becomes_an_id() {
    let session = require_session_id(&headers_with(SESSION_ID, "  sess_abc  "))
        .expect("a padded but non-empty header is valid");

    assert_eq!(session.as_str(), "sess_abc");
}

#[test]
fn an_absent_conversation_header_is_not_an_error() {
    let resolved = optional_gateway_conversation_id(&HeaderMap::new())
        .expect("the conversation header is optional");

    assert_eq!(resolved, None);
}

#[test]
fn a_blank_conversation_header_is_treated_as_absent() {
    let resolved = optional_gateway_conversation_id(&headers_with(GATEWAY_CONVERSATION_ID, "  "))
        .expect("a blank conversation header is not a client error");

    assert_eq!(resolved, None);
}

#[test]
fn a_present_conversation_header_is_trimmed_and_returned() {
    let resolved = optional_gateway_conversation_id(&headers_with(
        GATEWAY_CONVERSATION_ID,
        "  ctx_0123456789abcdef  ",
    ))
    .expect("a well-formed conversation header is accepted")
    .expect("a non-blank header yields an id");

    assert_eq!(resolved.as_str(), "ctx_0123456789abcdef");
}

#[test]
fn a_conversation_header_that_is_not_a_ctx_id_is_a_400() {
    let (status, message) =
        optional_gateway_conversation_id(&headers_with(GATEWAY_CONVERSATION_ID, "conv-1"))
            .expect_err("the conversation header is a validated typed id");

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message.contains(GATEWAY_CONVERSATION_ID), "{message}");
}

#[tokio::test]
async fn an_unparseable_body_is_a_400_and_still_records_the_raw_bytes() {
    let mut partial = RejectionPartial::default();

    let (status, message) = read_gateway_body(&inbound(), post("not json"), &mut partial)
        .await
        .expect_err("a non-JSON body cannot become a canonical request");

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message.contains("invalid request body"), "{message}");
    assert_eq!(
        partial.body.as_deref(),
        Some(b"not json".as_slice()),
        "the raw body must be captured for the audit row even when parsing fails"
    );
}

#[tokio::test]
async fn a_parsed_body_populates_the_audit_partial_from_the_canonical_request() {
    let mut partial = RejectionPartial::default();
    let body = r#"{"model":"claude-test","max_tokens":16,"stream":true,
        "messages":[{"role":"user","content":"hi"}]}"#;

    let (raw, request) = read_gateway_body(&inbound(), post(body), &mut partial)
        .await
        .expect("a well-formed Anthropic Messages body must parse");

    assert_eq!(raw.len(), body.len());
    assert_eq!(request.model, "claude-test");
    assert_eq!(partial.model.as_deref(), Some("claude-test"));
    assert_eq!(partial.max_tokens, Some(16));
    assert!(partial.is_streaming);
}

#[test]
fn a_header_supplied_conversation_id_wins_over_derivation() {
    let mut partial = RejectionPartial::default();
    let supplied = GatewayConversationId::try_new("ctx_00000000deadbeef".to_owned())
        .expect("test conversation id must be valid");

    let (conversation, context, _) = derive_conversation(
        &systemprompt_identifiers::UserId::new_unchecked("owner-a"),
        Some(supplied.clone()),
        &canonical(vec![user_message("hello")]),
        &mut partial,
    )
    .expect("an explicit conversation id is always usable");

    assert_eq!(conversation, supplied);
    assert_eq!(partial.gateway_conversation_id, Some(supplied));
    assert_eq!(partial.context_id, Some(context));
}

#[test]
fn a_conversation_id_is_derived_from_the_message_history_when_no_header_is_sent() {
    let mut partial = RejectionPartial::default();

    let (conversation, _, _) = derive_conversation(
        &systemprompt_identifiers::UserId::new_unchecked("owner-a"),
        None,
        &canonical(vec![user_message("hello")]),
        &mut partial,
    )
    .expect("a request with messages can derive its conversation");

    let (repeat, _, _) = derive_conversation(
        &systemprompt_identifiers::UserId::new_unchecked("owner-a"),
        None,
        &canonical(vec![user_message("hello")]),
        &mut RejectionPartial::default(),
    )
    .expect("derivation must succeed again");

    assert_eq!(
        conversation, repeat,
        "derivation must be stable so a retried request joins the same conversation"
    );
    assert_eq!(partial.gateway_conversation_id, Some(conversation));
}

const CLAUDE_CODE_USER_ID: &str = "user_1f8e6b2d9c4a7e0f1b3d5a7c9e2f4b6d8a0c2e4f6b8d0a2c4e6f8a0b2c4d6e8f_account_3c9b1d2e-7f4a-4b6c-9d8e-0f1a2b3c4d5e_session_9d2c4e6f-1a3b-4c5d-8e7f-0a1b2c3d4e5f";

fn canonical_from_claude_code(messages: Vec<CanonicalMessage>) -> CanonicalRequest {
    let mut request = canonical(messages);
    request.metadata = Some(serde_json::json!({ "user_id": CLAUDE_CODE_USER_ID }));
    request
}

#[test]
fn a_client_session_in_metadata_selects_the_hook_sessions_context() {
    let mut partial = RejectionPartial::default();

    let (conversation, context, client_session) = derive_conversation(
        &systemprompt_identifiers::UserId::new_unchecked("owner-a"),
        None,
        &canonical_from_claude_code(vec![user_message("hello")]),
        &mut partial,
    )
    .expect("derivation must succeed");

    let expected_session = ClientSessionId::try_new("9d2c4e6f-1a3b-4c5d-8e7f-0a1b2c3d4e5f")
        .expect("test session id must be valid");
    assert_eq!(client_session.as_ref(), Some(&expected_session));
    assert_eq!(
        context,
        ContextId::derived_from_session(&SessionId::new(expected_session.as_str())),
        "the gateway context must be the one the hooks pipeline derives from the same uuid"
    );
    assert_ne!(
        context,
        ContextId::derived_from_gateway_conversation(
            &systemprompt_identifiers::UserId::new_unchecked("owner-a"),
            &conversation
        ),
        "the context no longer follows the prefix hash once the caller names its session"
    );
    assert_eq!(partial.client_session_id, Some(expected_session));
    assert_eq!(partial.context_id, Some(context));
}

#[test]
fn a_header_supplied_conversation_id_pins_the_context_even_with_a_client_session() {
    let mut partial = RejectionPartial::default();
    let supplied = GatewayConversationId::try_new("ctx_00000000deadbeef".to_owned())
        .expect("test conversation id must be valid");

    let (_, context, client_session) = derive_conversation(
        &systemprompt_identifiers::UserId::new_unchecked("owner-a"),
        Some(supplied.clone()),
        &canonical_from_claude_code(vec![user_message("hello")]),
        &mut partial,
    )
    .expect("an explicit conversation id is always usable");

    assert!(
        client_session.is_some(),
        "the session is still parsed and recorded"
    );
    assert_eq!(
        context,
        ContextId::derived_from_gateway_conversation(
            &systemprompt_identifiers::UserId::new_unchecked("owner-a"),
            &supplied
        )
    );
}

#[test]
fn a_request_without_metadata_keeps_the_prefix_hash_context() {
    let mut partial = RejectionPartial::default();

    let (conversation, context, client_session) = derive_conversation(
        &systemprompt_identifiers::UserId::new_unchecked("owner-a"),
        None,
        &canonical(vec![user_message("hello")]),
        &mut partial,
    )
    .expect("derivation must succeed");

    assert!(client_session.is_none());
    assert_eq!(
        context,
        ContextId::derived_from_gateway_conversation(
            &systemprompt_identifiers::UserId::new_unchecked("owner-a"),
            &conversation
        )
    );
    assert_eq!(partial.client_session_id, None);
}

#[test]
fn a_body_with_no_messages_cannot_derive_a_conversation() {
    let mut partial = RejectionPartial::default();

    let (status, message) = derive_conversation(
        &systemprompt_identifiers::UserId::new_unchecked("owner-a"),
        None,
        &canonical(vec![]),
        &mut partial,
    )
    .expect_err("there is nothing to derive a conversation from");

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message.contains("cannot derive"), "{message}");
    assert_eq!(partial.gateway_conversation_id, None);
}

#[test]
fn the_error_body_escapes_quotes_so_the_envelope_stays_valid_json() {
    let response = build_error_response(
        StatusCode::FORBIDDEN,
        "permission_error",
        r#"model "x" is \denied"#,
    );

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .expect("the error envelope is JSON"),
        "application/json"
    );
}

#[tokio::test]
async fn the_error_body_is_a_parseable_error_envelope() {
    let response = build_error_response(
        StatusCode::BAD_GATEWAY,
        "api_error",
        r#"a "quoted" failure"#,
    );
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("the error body is small and fully buffered");
    let parsed: serde_json::Value =
        serde_json::from_slice(&bytes).expect("the escaped message must still yield valid JSON");

    assert_eq!(parsed["type"], "error");
    assert_eq!(parsed["error"]["type"], "api_error");
    assert_eq!(parsed["error"]["message"], r#"a "quoted" failure"#);
}

fn raw_header(name: &'static str, bytes: &[u8]) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        name,
        HeaderValue::from_bytes(bytes).expect("a header value of arbitrary bytes is constructible"),
    );
    headers
}

#[test]
fn a_session_header_that_is_not_utf8_is_a_400_rather_than_a_panic() {
    let headers = raw_header(SESSION_ID, &[0xC3, 0x28]);

    let (status, message) =
        require_session_id(&headers).expect_err("a non-UTF-8 header value cannot name a session");

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message.contains(SESSION_ID), "{message}");
    assert!(message.contains("invalid"), "{message}");
}

#[test]
fn a_conversation_header_that_is_not_utf8_is_a_400() {
    let headers = raw_header(GATEWAY_CONVERSATION_ID, &[0xFF, 0xFE]);

    let (status, message) = optional_gateway_conversation_id(&headers)
        .expect_err("a non-UTF-8 conversation header cannot be decoded");

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message.contains(GATEWAY_CONVERSATION_ID), "{message}");
}

#[tokio::test]
async fn a_body_over_the_buffer_limit_is_rejected_rather_than_buffered() {
    let oversized = vec![b'x'; systemprompt_models::wire::BUFFERED_BODY_LIMIT_BYTES + 1];
    let request = Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .body(Body::from(oversized))
        .expect("test request must build");
    let mut partial = RejectionPartial::default();

    let (status, message) = read_gateway_body(&inbound(), request, &mut partial)
        .await
        .expect_err("a body past the buffer limit must not be read into memory");

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message.contains("failed to read request body"), "{message}");
    assert!(
        partial.body.is_none(),
        "an unread body must not be recorded on the rejection partial"
    );
}

#[test]
fn identical_fallback_conversations_have_distinct_authenticated_owner_contexts() {
    let alice = systemprompt_identifiers::UserId::generate();
    let bob = systemprompt_identifiers::UserId::generate();
    let request = canonical(vec![user_message("same opening message")]);
    let (alice_gateway, alice_context, _) =
        derive_conversation(&alice, None, &request, &mut RejectionPartial::default())
            .expect("alice conversation");
    let (bob_gateway, bob_context, _) =
        derive_conversation(&bob, None, &request, &mut RejectionPartial::default())
            .expect("bob conversation");
    assert_eq!(alice_gateway, bob_gateway);
    assert_ne!(alice_context, bob_context);
}
