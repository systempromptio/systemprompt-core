//! Tests for bridge session-exchange pure helpers and data types.
//!
//! The DB-bound issue/exchange/provision entry points require a live
//! `DbPool` and `AnalyticsProvider` chain, so they live in integration
//! tests. The helpers exercised here are pure: SHA-256 hex hashing,
//! struct construction, and Debug/Clone/Serialize impls.

use std::collections::HashMap;

use systemprompt_identifiers::ClientId;
use systemprompt_oauth::services::bridge::{
    BridgeAuthResult, BridgeExchangeCode, BridgeOAuthClient, hash_exchange_code,
};

#[test]
fn hash_exchange_code_is_64_hex_chars() {
    let hash = hash_exchange_code("any-code");

    assert_eq!(hash.len(), 64);
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(hash.chars().all(|c| !c.is_ascii_uppercase()));
}

#[test]
fn hash_exchange_code_is_deterministic() {
    let a = hash_exchange_code("repeatable-input");
    let b = hash_exchange_code("repeatable-input");

    assert_eq!(a, b);
}

#[test]
fn hash_exchange_code_differs_per_input() {
    let a = hash_exchange_code("alpha");
    let b = hash_exchange_code("beta");

    assert_ne!(a, b);
}

#[test]
fn hash_exchange_code_handles_empty_input() {
    // SHA-256 of the empty string is a well-known constant; we only assert
    // the function does not panic and returns the canonical length.
    let hash = hash_exchange_code("");

    assert_eq!(hash.len(), 64);
    assert_eq!(
        hash,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn hash_exchange_code_handles_long_input() {
    let long = "x".repeat(4096);
    let hash = hash_exchange_code(&long);

    assert_eq!(hash.len(), 64);
}

#[test]
fn bridge_auth_result_round_trips_through_serde() {
    let mut headers = HashMap::new();
    headers.insert("x-user-id".to_string(), "user_1".to_string());

    let result = BridgeAuthResult {
        token: "jwt.token.value".to_string(),
        ttl: 3600,
        headers,
    };

    let json = serde_json::to_string(&result).expect("serialize bridge auth result");

    assert!(json.contains("\"token\":\"jwt.token.value\""));
    assert!(json.contains("\"ttl\":3600"));
    assert!(json.contains("\"x-user-id\""));
}

#[test]
fn bridge_auth_result_clone_preserves_fields() {
    let mut headers = HashMap::new();
    headers.insert("k".to_string(), "v".to_string());

    let original = BridgeAuthResult {
        token: "tok".to_string(),
        ttl: 60,
        headers,
    };
    let cloned = original.clone();

    assert_eq!(cloned.token, original.token);
    assert_eq!(cloned.ttl, original.ttl);
    assert_eq!(cloned.headers, original.headers);
}

#[test]
fn bridge_exchange_code_debug_contains_code() {
    let issued = BridgeExchangeCode {
        code: "abc123".to_string(),
        expires_at: chrono::Utc::now(),
    };

    let debug = format!("{:?}", issued);

    assert!(debug.contains("BridgeExchangeCode"));
    assert!(debug.contains("abc123"));
}

#[test]
fn bridge_exchange_code_serialises_iso_timestamp() {
    let issued = BridgeExchangeCode {
        code: "code-value".to_string(),
        expires_at: chrono::DateTime::from_timestamp(1_700_000_000, 0).expect("valid timestamp"),
    };

    let json = serde_json::to_string(&issued).expect("serialize exchange code");

    assert!(json.contains("\"code\":\"code-value\""));
    assert!(json.contains("2023"));
}

#[test]
fn bridge_oauth_client_serialises_secret() {
    let client = BridgeOAuthClient {
        client_id: ClientId::new("bridge:user_42"),
        client_secret: "secret_xyz".to_string(),
        scopes: vec!["hook:govern".to_string(), "hook:track".to_string()],
        token_endpoint: "https://example.test/oauth/token".to_string(),
    };

    let json = serde_json::to_string(&client).expect("serialize bridge oauth client");

    assert!(json.contains("\"client_id\":\"bridge:user_42\""));
    assert!(json.contains("\"client_secret\":\"secret_xyz\""));
    assert!(json.contains("hook:govern"));
    assert!(json.contains("https://example.test/oauth/token"));
}

#[test]
fn bridge_oauth_client_clone_preserves_all_fields() {
    let original = BridgeOAuthClient {
        client_id: ClientId::new("bridge:user_1"),
        client_secret: "s".to_string(),
        scopes: vec!["hook:govern".to_string()],
        token_endpoint: "https://t.test".to_string(),
    };

    let cloned = original.clone();

    assert_eq!(cloned.client_id.as_str(), original.client_id.as_str());
    assert_eq!(cloned.client_secret, original.client_secret);
    assert_eq!(cloned.scopes, original.scopes);
    assert_eq!(cloned.token_endpoint, original.token_endpoint);
}
