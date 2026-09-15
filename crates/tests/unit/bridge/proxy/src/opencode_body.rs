//! `stamp_opencode_session` moves the plugin's session UUID into the body the
//! gateway keys contexts on, and touches nothing else.

use bytes::Bytes;
use systemprompt_bridge::feedback::opencode_session::session_uuid;
use systemprompt_bridge::proxy::forward::{CHAT_COMPLETIONS_PATH, stamp_opencode_session};

const BODY: &str = r#"{"model":"claude-sonnet-5","messages":[{"role":"user","content":"hi"}]}"#;

fn with_session(uuid: &str) -> http::HeaderMap {
    let mut headers = http::HeaderMap::new();
    headers.insert("user-agent", "opencode/1.0".parse().unwrap());
    headers.insert("x-opencode-session", uuid.parse().unwrap());
    headers
}

#[test]
fn a_chat_request_with_the_header_gets_metadata_user_id() {
    let uuid = session_uuid("ses_abc").unwrap();
    let out = stamp_opencode_session(
        Bytes::from_static(BODY.as_bytes()),
        &with_session(uuid.as_str()),
        CHAT_COMPLETIONS_PATH,
    );
    let doc: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let user_id = doc["metadata"]["user_id"].as_str().expect("stamped");
    let parsed: serde_json::Value = serde_json::from_str(user_id).unwrap();
    assert_eq!(parsed["session_id"], uuid.as_str());
    assert_eq!(doc["model"], "claude-sonnet-5");
    assert_eq!(doc["messages"][0]["content"], "hi");
    assert_eq!(
        systemprompt_identifiers::ClientSessionId::from_metadata_user_id(user_id)
            .unwrap()
            .unwrap(),
        uuid,
        "the gateway parses the stamp back to the same session"
    );
}

#[test]
fn a_gateway_prefixed_chat_path_is_stamped_too() {
    let uuid = session_uuid("ses_abc").unwrap();
    let out = stamp_opencode_session(
        Bytes::from_static(BODY.as_bytes()),
        &with_session(uuid.as_str()),
        "/api/gateway/v1/chat/completions",
    );
    let doc: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert!(doc["metadata"]["user_id"].is_string(), "{doc}");
}

#[test]
fn without_the_header_the_body_is_byte_identical() {
    let out = stamp_opencode_session(
        Bytes::from_static(BODY.as_bytes()),
        &http::HeaderMap::new(),
        CHAT_COMPLETIONS_PATH,
    );
    assert_eq!(out.as_ref(), BODY.as_bytes());
}

#[test]
fn on_the_messages_path_the_body_is_byte_identical() {
    let uuid = session_uuid("ses_abc").unwrap();
    let out = stamp_opencode_session(
        Bytes::from_static(BODY.as_bytes()),
        &with_session(uuid.as_str()),
        "/v1/messages",
    );
    assert_eq!(out.as_ref(), BODY.as_bytes());
}

#[test]
fn a_raw_native_id_in_the_header_is_not_trusted_into_the_body() {
    let out = stamp_opencode_session(
        Bytes::from_static(BODY.as_bytes()),
        &with_session("ses_abc"),
        CHAT_COMPLETIONS_PATH,
    );
    assert_eq!(out.as_ref(), BODY.as_bytes());
}

#[test]
fn an_existing_metadata_user_id_is_kept() {
    let uuid = session_uuid("ses_abc").unwrap();
    let body = r#"{"messages":[],"metadata":{"user_id":"caller-owned"}}"#;
    let out = stamp_opencode_session(
        Bytes::from_static(body.as_bytes()),
        &with_session(uuid.as_str()),
        CHAT_COMPLETIONS_PATH,
    );
    assert_eq!(out.as_ref(), body.as_bytes());
}

#[test]
fn a_non_json_body_passes_through() {
    let uuid = session_uuid("ses_abc").unwrap();
    let out = stamp_opencode_session(
        Bytes::from_static(b"not json"),
        &with_session(uuid.as_str()),
        CHAT_COMPLETIONS_PATH,
    );
    assert_eq!(out.as_ref(), b"not json");
}
