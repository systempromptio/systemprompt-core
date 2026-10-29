//! The private device token a bridge exchange returns stays with the bridge:
//! it survives into the bridge's own token cache and never into what a host
//! is handed.

use systemprompt_bridge::gateway::types::{AuthResponse, HelperOutput};

#[test]
fn the_device_token_is_kept_but_never_handed_to_a_host() {
    let response: AuthResponse = serde_json::from_value(serde_json::json!({
        "token": "host-token",
        "ttl": 3600,
        "headers": {},
        "device_token": "device-token",
    }))
    .expect("an exchange answer");
    let kept = HelperOutput::from(response);
    assert_eq!(
        kept.device_token.as_ref().map(|t| t.expose().to_owned()),
        Some("device-token".to_owned())
    );
    let cached = serde_json::to_value(&kept).expect("cache entry");
    assert_eq!(
        cached["device_token"], "device-token",
        "the 0600 cache keeps it"
    );

    let handed = serde_json::to_value(kept.for_host()).expect("helper output");
    assert_eq!(handed["token"], "host-token");
    assert!(handed.get("device_token").is_none(), "{handed}");
}

#[test]
fn an_older_server_without_a_device_token_still_signs_in() {
    let response: AuthResponse = serde_json::from_value(serde_json::json!({
        "token": "host-token",
        "ttl": 3600,
    }))
    .expect("an exchange answer without a device token");
    assert!(HelperOutput::from(response).device_token.is_none());
}
