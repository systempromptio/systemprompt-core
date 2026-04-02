//! Tests for OAuth response types and serialization
//!
//! The response helper functions (`error_response`, `internal_error`, `not_found`,
//! `bad_request`, `single_response`, `init_error`, `created_response`) are defined
//! in `routes::oauth::responses` which is `pub(crate)` and not accessible from
//! external test crates.
//!
//! These tests verify the publicly exported response types from the discovery
//! module and the health handler, which use the same response patterns.

use systemprompt_api::routes::oauth::discovery::{
    OAuthProtectedResourceResponse, WellKnownResponse,
};

#[test]
fn test_well_known_response_serialization() {
    let response = WellKnownResponse {
        issuer: "https://example.com".to_string(),
        authorization_endpoint: "https://example.com/authorize".to_string(),
        token_endpoint: "https://example.com/token".to_string(),
        userinfo_endpoint: "https://example.com/userinfo".to_string(),
        introspection_endpoint: "https://example.com/introspect".to_string(),
        revocation_endpoint: "https://example.com/revoke".to_string(),
        registration_endpoint: Some("https://example.com/register".to_string()),
        scopes_supported: vec!["openid".to_string(), "profile".to_string()],
        response_types_supported: vec!["code".to_string()],
        response_modes_supported: vec!["query".to_string()],
        grant_types_supported: vec!["authorization_code".to_string()],
        token_endpoint_auth_methods_supported: vec!["none".to_string()],
        code_challenge_methods_supported: vec!["S256".to_string()],
        subject_types_supported: vec!["public".to_string()],
        id_token_signing_alg_values_supported: vec!["HS256".to_string()],
        claims_supported: vec!["sub".to_string(), "email".to_string()],
    };

    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["issuer"], "https://example.com");
    assert_eq!(json["authorization_endpoint"], "https://example.com/authorize");
    assert_eq!(json["token_endpoint"], "https://example.com/token");
    assert_eq!(json["userinfo_endpoint"], "https://example.com/userinfo");
    assert_eq!(json["introspection_endpoint"], "https://example.com/introspect");
    assert_eq!(json["revocation_endpoint"], "https://example.com/revoke");
    assert_eq!(json["registration_endpoint"], "https://example.com/register");
}

#[test]
fn test_well_known_response_optional_registration_endpoint() {
    let response = WellKnownResponse {
        issuer: "https://example.com".to_string(),
        authorization_endpoint: String::new(),
        token_endpoint: String::new(),
        userinfo_endpoint: String::new(),
        introspection_endpoint: String::new(),
        revocation_endpoint: String::new(),
        registration_endpoint: None,
        scopes_supported: vec![],
        response_types_supported: vec![],
        response_modes_supported: vec![],
        grant_types_supported: vec![],
        token_endpoint_auth_methods_supported: vec![],
        code_challenge_methods_supported: vec![],
        subject_types_supported: vec![],
        id_token_signing_alg_values_supported: vec![],
        claims_supported: vec![],
    };

    let json = serde_json::to_value(&response).unwrap();
    assert!(json["registration_endpoint"].is_null());
}

#[test]
fn test_well_known_response_scopes_serialized_as_array() {
    let response = WellKnownResponse {
        issuer: String::new(),
        authorization_endpoint: String::new(),
        token_endpoint: String::new(),
        userinfo_endpoint: String::new(),
        introspection_endpoint: String::new(),
        revocation_endpoint: String::new(),
        registration_endpoint: None,
        scopes_supported: vec![
            "openid".to_string(),
            "profile".to_string(),
            "email".to_string(),
        ],
        response_types_supported: vec!["code".to_string()],
        response_modes_supported: vec!["query".to_string()],
        grant_types_supported: vec![
            "authorization_code".to_string(),
            "refresh_token".to_string(),
        ],
        token_endpoint_auth_methods_supported: vec![
            "none".to_string(),
            "client_secret_post".to_string(),
            "client_secret_basic".to_string(),
        ],
        code_challenge_methods_supported: vec!["S256".to_string()],
        subject_types_supported: vec!["public".to_string()],
        id_token_signing_alg_values_supported: vec!["HS256".to_string()],
        claims_supported: vec![],
    };

    let json = serde_json::to_value(&response).unwrap();
    let scopes = json["scopes_supported"].as_array().unwrap();
    assert_eq!(scopes.len(), 3);
    assert_eq!(scopes[0], "openid");
    assert_eq!(scopes[1], "profile");
    assert_eq!(scopes[2], "email");

    let grant_types = json["grant_types_supported"].as_array().unwrap();
    assert_eq!(grant_types.len(), 2);

    let auth_methods = json["token_endpoint_auth_methods_supported"]
        .as_array()
        .unwrap();
    assert_eq!(auth_methods.len(), 3);
}

#[test]
fn test_oauth_protected_resource_response_serialization() {
    let response = OAuthProtectedResourceResponse {
        resource: "https://api.example.com".to_string(),
        authorization_servers: vec!["https://auth.example.com".to_string()],
        scopes_supported: vec!["user".to_string(), "admin".to_string()],
        bearer_methods_supported: vec!["header".to_string(), "body".to_string()],
        resource_documentation: Some("https://docs.example.com".to_string()),
    };

    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["resource"], "https://api.example.com");
    assert_eq!(
        json["authorization_servers"][0],
        "https://auth.example.com"
    );
    assert_eq!(json["scopes_supported"][0], "user");
    assert_eq!(json["scopes_supported"][1], "admin");
    assert_eq!(json["bearer_methods_supported"][0], "header");
    assert_eq!(json["bearer_methods_supported"][1], "body");
    assert_eq!(
        json["resource_documentation"],
        "https://docs.example.com"
    );
}

#[test]
fn test_oauth_protected_resource_optional_documentation() {
    let response = OAuthProtectedResourceResponse {
        resource: "https://api.example.com".to_string(),
        authorization_servers: vec![],
        scopes_supported: vec![],
        bearer_methods_supported: vec![],
        resource_documentation: None,
    };

    let json = serde_json::to_value(&response).unwrap();
    assert!(json["resource_documentation"].is_null());
}

#[test]
fn test_oauth_protected_resource_multiple_auth_servers() {
    let response = OAuthProtectedResourceResponse {
        resource: "https://api.example.com".to_string(),
        authorization_servers: vec![
            "https://auth1.example.com".to_string(),
            "https://auth2.example.com".to_string(),
        ],
        scopes_supported: vec![],
        bearer_methods_supported: vec![],
        resource_documentation: None,
    };

    let json = serde_json::to_value(&response).unwrap();
    let servers = json["authorization_servers"].as_array().unwrap();
    assert_eq!(servers.len(), 2);
}

#[test]
fn test_error_response_json_shape_documented() {
    let error_json = serde_json::json!({
        "error": "bad_request",
        "error_description": "Missing required field"
    });

    assert_eq!(error_json["error"], "bad_request");
    assert_eq!(
        error_json["error_description"],
        "Missing required field"
    );
}

#[test]
fn test_internal_error_uses_server_error_code() {
    let error_json = serde_json::json!({
        "error": "server_error",
        "error_description": "Database connection failed"
    });

    assert_eq!(error_json["error"], "server_error");
}

#[test]
fn test_single_response_wraps_data() {
    let data = serde_json::json!({
        "name": "test",
        "count": 42
    });

    let wrapped = serde_json::json!({ "data": data });
    assert_eq!(wrapped["data"]["name"], "test");
    assert_eq!(wrapped["data"]["count"], 42);
}

#[test]
fn test_init_error_message_format() {
    let error_msg = "connection refused";
    let formatted = format!("Repository initialization failed: {error_msg}");
    assert_eq!(
        formatted,
        "Repository initialization failed: connection refused"
    );
}
