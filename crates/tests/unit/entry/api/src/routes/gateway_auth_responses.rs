//! Bridge auth response DTOs and the `capabilities()` handler
//! (`routes::gateway::auth`).
//!
//! Covers the pure surface: `Capabilities` advertisement, `AuthResponse`
//! serialization, and the `From<BridgeAuthResult>` mapping. The token-minting
//! handlers themselves need an `AppContext` and are out of scope here.

use std::collections::HashMap;
use systemprompt_api::routes::gateway::auth::{AuthResponse, Capabilities, capabilities};
use systemprompt_oauth::services::BridgeAuthResult;

#[tokio::test]
async fn capabilities_handler_advertises_all_modes() {
    let json = capabilities().await;
    let modes = &json.0.modes;
    assert!(modes.contains(&"pat"));
    assert!(modes.contains(&"session"));
    assert!(modes.contains(&"mtls"));
    assert!(modes.contains(&"oauth-client"));
    assert_eq!(modes.len(), 4);
}

#[test]
fn capabilities_serializes_modes_array() {
    let caps = Capabilities {
        modes: vec!["pat", "session"],
    };
    let v = serde_json::to_value(&caps).expect("serialize");
    assert_eq!(v["modes"][0], "pat");
    assert_eq!(v["modes"][1], "session");
}

#[test]
fn auth_response_serializes_token_ttl_headers() {
    let mut headers = HashMap::new();
    headers.insert("x-systemprompt-token".to_owned(), "abc".to_owned());
    let resp = AuthResponse {
        token: "jwt-value".to_owned(),
        ttl: 3600,
        headers,
    };
    let v = serde_json::to_value(&resp).expect("serialize");
    assert_eq!(v["token"], "jwt-value");
    assert_eq!(v["ttl"], 3600);
    assert_eq!(v["headers"]["x-systemprompt-token"], "abc");
}

#[test]
fn auth_response_from_bridge_result_maps_fields() {
    let mut headers = HashMap::new();
    headers.insert("authorization".to_owned(), "Bearer t".to_owned());
    let bridge = BridgeAuthResult {
        token: "tok".to_owned(),
        ttl: 900,
        headers: headers.clone(),
    };
    let resp = AuthResponse::from(bridge);
    assert_eq!(resp.token, "tok");
    assert_eq!(resp.ttl, 900);
    assert_eq!(resp.headers, headers);
}

#[test]
fn auth_response_from_bridge_result_preserves_empty_headers() {
    let bridge = BridgeAuthResult {
        token: "t".to_owned(),
        ttl: 0,
        headers: HashMap::new(),
    };
    let resp = AuthResponse::from(bridge);
    assert!(resp.headers.is_empty());
    assert_eq!(resp.ttl, 0);
}
