use systemprompt_bridge::feedback::opencode_session::{OPENCODE_SESSION_NAMESPACE, session_uuid};
use systemprompt_identifiers::ClientSessionId;

#[test]
fn session_uuid_is_deterministic_and_distinct_per_native_id() {
    let first = session_uuid("ses_abc123").unwrap();
    let again = session_uuid("ses_abc123").unwrap();
    let other = session_uuid("ses_abc124").unwrap();
    assert_eq!(first, again);
    assert_ne!(first, other);
}

#[test]
fn session_uuid_is_a_lowercase_hyphenated_v5_uuid() {
    let id = session_uuid("ses_abc123").unwrap();
    let text = id.as_str();
    assert_eq!(text.len(), 36, "{text}");
    assert_eq!(text, text.to_ascii_lowercase());
    let bytes = text.as_bytes();
    for index in [8, 13, 18, 23] {
        assert_eq!(bytes[index], b'-', "{text}");
    }
    assert_eq!(bytes[14], b'5', "version nibble must be 5: {text}");
    assert!(
        matches!(bytes[19], b'8' | b'9' | b'a' | b'b'),
        "RFC 4122 variant bits: {text}"
    );
}

#[test]
fn session_uuid_round_trips_through_metadata_user_id() {
    let id = session_uuid("ses_abc123").unwrap();
    let metadata = serde_json::json!({ "session_id": id.as_str() }).to_string();
    let parsed = ClientSessionId::from_metadata_user_id(&metadata)
        .unwrap()
        .expect("metadata carries a session");
    assert_eq!(parsed, id);
    assert_eq!(ClientSessionId::try_new(id.as_str()).unwrap(), id);
}

#[test]
fn the_namespace_is_pinned() {
    // Why: the same string is baked into the emitted OpenCode plugin; moving
    // it would split every open session across two contexts.
    assert_eq!(
        OPENCODE_SESSION_NAMESPACE.hyphenated().to_string(),
        "7c1f5b6e-3a2d-4e8f-9b0c-2d6a1e4f8c73"
    );
}
