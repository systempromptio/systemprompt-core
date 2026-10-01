//! Bridge-auth response shapes and the messaging reply-surface renderer.
//!
//! `AuthResponse: From<BridgeAuthResult>` is the only place a minted bridge
//! credential becomes a wire body, and `MessagingError::user_message` is the
//! only text a Slack or Teams user ever sees when a dispatch fails. Neither is
//! called by any test.

use std::collections::HashMap;

use systemprompt_api::routes::gateway::auth::{AuthResponse, capabilities};
use systemprompt_api::routes::messaging::MessagingError;
use systemprompt_oauth::services::BridgeAuthResult;

#[test]
fn a_minted_bridge_credential_becomes_the_wire_body_intact() {
    let mut headers = HashMap::new();
    headers.insert("x-session-id".to_owned(), "sess-1".to_owned());
    let result = BridgeAuthResult {
        token: "minted-token".to_owned(),
        ttl: 3600,
        headers,
    };

    let response: AuthResponse = result.into();

    assert_eq!(response.token, "minted-token");
    assert_eq!(
        response.ttl, 3600,
        "the client schedules its refresh off this value"
    );
    assert_eq!(
        response.headers.get("x-session-id").map(String::as_str),
        Some("sess-1"),
        "the session binding headers must survive the conversion"
    );
}

#[tokio::test]
async fn the_advertised_auth_modes_are_the_ones_the_bridge_can_use() {
    let modes = capabilities().await.0.modes;

    for expected in ["pat", "session", "oauth-client"] {
        assert!(
            modes.contains(&expected),
            "the bridge negotiates against this list; {expected} is missing: {modes:?}"
        );
    }
}

#[test]
fn a_dispatch_failure_renders_one_apologetic_line_per_variant() {
    let variants = [
        MessagingError::AgentRejected {
            code: -32603,
            message: "agent unreachable".to_owned(),
        },
        MessagingError::Response(
            serde_json::from_str::<serde_json::Value>("not json").unwrap_err(),
        ),
        MessagingError::Encode(serde_json::from_str::<serde_json::Value>("{").unwrap_err()),
    ];

    for err in variants {
        let rendered = err.user_message();
        assert!(
            rendered.starts_with("Sorry — something went wrong handling that."),
            "every failure gets the same opaque opener: {rendered}"
        );
    }
}

#[test]
fn the_reply_surface_never_names_the_failing_subsystem_to_the_user() {
    let err = MessagingError::AgentRejected {
        code: -32000,
        message: "vault credentials expired".to_owned(),
    };

    let rendered = err.user_message();
    assert_eq!(rendered, "Sorry — something went wrong handling that.");
    assert!(!rendered.contains("vault credentials expired"));

    assert!(err.to_string().contains("vault credentials expired"));
}
