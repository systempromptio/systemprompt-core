//! Bridge-auth response shapes.
//!
//! `AuthResponse: From<BridgeAuthResult>` is the only place a minted bridge
//! credential becomes a wire body.

use std::collections::HashMap;

use systemprompt_api::routes::gateway::auth::{AuthResponse, capabilities};
use systemprompt_oauth::services::BridgeAuthResult;

#[test]
fn a_minted_bridge_credential_becomes_the_wire_body_intact() {
    let mut headers = HashMap::new();
    headers.insert("x-session-id".to_owned(), "sess-1".to_owned());
    let result = BridgeAuthResult {
        token: "minted-token".to_owned(),
        ttl: 3600,
        headers,
        device_token: None,
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
