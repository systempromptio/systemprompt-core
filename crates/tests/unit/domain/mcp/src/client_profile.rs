//! Rebuilding a client profile from a recovered session.
//!
//! A session's `initialize` params are persisted so a recovered session can
//! shape its wire the same way the live one did. What matters is the failure
//! path: params that no longer parse must yield an unknown client rather than
//! failing the recovery. An unknown client is served the conservative wire, so
//! the worst case is a downgrade — where an error would drop the session and a
//! permissive default would send content the client cannot read.

use systemprompt_mcp::client_profile_from_stored;

#[test]
fn stored_initialize_params_rebuild_the_client_that_sent_them() {
    let stored = serde_json::json!({
        "protocolVersion": "2025-06-18",
        "capabilities": {},
        "clientInfo": {"name": "cowork", "version": "1.2.3"}
    });

    let profile = client_profile_from_stored(&stored);

    assert_eq!(profile.client_name.as_deref(), Some("cowork"));
    assert!(
        profile.protocol_version.is_some(),
        "a recovered session must remember which protocol it negotiated"
    );
}

// Why: this is the fail-safe. A row written by an older build, truncated, or
// hand-edited must not take the session down — and must not be read as a
// capable client either.
#[test]
fn params_that_do_not_parse_yield_an_unknown_client_rather_than_failing() {
    for unparseable in [
        serde_json::json!({}),
        serde_json::json!(null),
        serde_json::json!("not an object"),
        serde_json::json!({"protocolVersion": 42}),
        serde_json::json!({"clientInfo": {"name": "no version"}}),
    ] {
        let profile = client_profile_from_stored(&unparseable);

        assert!(
            profile.client_name.is_none(),
            "{unparseable} should not have produced a named client"
        );
        assert!(
            !profile.supports_ui(),
            "an unreadable session must not be sent UI artifacts"
        );
        assert!(
            !profile.supports_structured_content(),
            "an unreadable session must not be sent structured content"
        );
    }
}

// Why: the announced extensions decide what the wire may carry. Losing them on
// recovery silently downgrades a client that had asked for more.
#[test]
fn announced_extensions_survive_the_round_trip() {
    let stored = serde_json::json!({
        "protocolVersion": "2025-06-18",
        "capabilities": {
            "extensions": {"io.modelcontextprotocol/ui": {}}
        },
        "clientInfo": {"name": "cowork", "version": "1.2.3"}
    });

    let profile = client_profile_from_stored(&stored);

    assert!(
        profile.supports_ui(),
        "a session that negotiated UI keeps it across recovery: {profile:?}"
    );
}

// Why: a client that never asked for UI must not acquire it by being
// recovered. The absence has to survive as faithfully as the presence.
#[test]
fn a_client_that_announced_no_extensions_does_not_gain_them_on_recovery() {
    let stored = serde_json::json!({
        "protocolVersion": "2025-06-18",
        "capabilities": {},
        "clientInfo": {"name": "plain", "version": "1.0.0"}
    });

    let profile = client_profile_from_stored(&stored);

    assert_eq!(profile.client_name.as_deref(), Some("plain"));
    assert!(profile.extensions.is_empty());
    assert!(!profile.supports_ui());
}
