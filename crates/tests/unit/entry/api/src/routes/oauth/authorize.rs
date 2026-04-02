//! Unit tests for OAuth authorize endpoint types
//!
//! The validation and response_builder modules inside the authorize endpoint
//! are private (not `pub mod`), so their functions are not directly testable.
//! These tests cover the public data types: AuthorizeQuery, AuthorizeRequest,
//! and AuthorizeResponse - verifying construction, serde behavior, and Debug.

use systemprompt_api::routes::oauth::endpoints::authorize::{
    AuthorizeQuery, AuthorizeRequest, AuthorizeResponse,
};
use systemprompt_identifiers::ClientId;

// ============================================================================
// Helper
// ============================================================================

fn create_valid_authorize_query() -> AuthorizeQuery {
    AuthorizeQuery {
        response_type: "code".to_string(),
        client_id: ClientId::new("sp_test_client"),
        redirect_uri: Some("https://example.com/callback".to_string()),
        scope: Some("openid".to_string()),
        state: Some("random_state_value".to_string()),
        code_challenge: Some("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".to_string()),
        code_challenge_method: Some("S256".to_string()),
        response_mode: None,
        display: None,
        prompt: None,
        max_age: None,
        ui_locales: None,
        resource: None,
    }
}

fn create_valid_authorize_request() -> AuthorizeRequest {
    AuthorizeRequest {
        response_type: "code".to_string(),
        client_id: ClientId::new("sp_test_client"),
        redirect_uri: Some("https://example.com/callback".to_string()),
        scope: Some("openid".to_string()),
        state: Some("random_state_value".to_string()),
        code_challenge: Some("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".to_string()),
        code_challenge_method: Some("S256".to_string()),
        user_consent: Some("allow".to_string()),
        username: Some("testuser".to_string()),
        password: Some("testpass".to_string()),
        resource: None,
    }
}

// ============================================================================
// AuthorizeQuery Deserialization
// ============================================================================

#[test]
fn test_authorize_query_deserialize_all_fields() {
    let json = serde_json::json!({
        "response_type": "code",
        "client_id": "sp_test_client",
        "redirect_uri": "https://example.com/callback",
        "scope": "openid profile",
        "state": "abc123",
        "code_challenge": "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
        "code_challenge_method": "S256",
        "response_mode": "query",
        "display": "page",
        "prompt": "consent",
        "max_age": 3600,
        "ui_locales": "en",
        "resource": "https://api.example.com"
    });

    let query: AuthorizeQuery = serde_json::from_value(json).expect("should deserialize");

    assert_eq!(query.response_type, "code");
    assert_eq!(query.client_id.as_str(), "sp_test_client");
    assert_eq!(query.redirect_uri.as_deref(), Some("https://example.com/callback"));
    assert_eq!(query.scope.as_deref(), Some("openid profile"));
    assert_eq!(query.state.as_deref(), Some("abc123"));
    assert_eq!(
        query.code_challenge.as_deref(),
        Some("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk")
    );
    assert_eq!(query.code_challenge_method.as_deref(), Some("S256"));
    assert_eq!(query.response_mode.as_deref(), Some("query"));
    assert_eq!(query.display.as_deref(), Some("page"));
    assert_eq!(query.prompt.as_deref(), Some("consent"));
    assert_eq!(query.max_age, Some(3600));
    assert_eq!(query.ui_locales.as_deref(), Some("en"));
    assert_eq!(query.resource.as_deref(), Some("https://api.example.com"));
}

#[test]
fn test_authorize_query_deserialize_required_fields_only() {
    let json = serde_json::json!({
        "response_type": "code",
        "client_id": "sp_test_client"
    });

    let query: AuthorizeQuery = serde_json::from_value(json).expect("should deserialize");

    assert_eq!(query.response_type, "code");
    assert_eq!(query.client_id.as_str(), "sp_test_client");
    assert!(query.redirect_uri.is_none());
    assert!(query.scope.is_none());
    assert!(query.state.is_none());
    assert!(query.code_challenge.is_none());
    assert!(query.code_challenge_method.is_none());
    assert!(query.response_mode.is_none());
    assert!(query.display.is_none());
    assert!(query.prompt.is_none());
    assert!(query.max_age.is_none());
    assert!(query.ui_locales.is_none());
    assert!(query.resource.is_none());
}

#[test]
fn test_authorize_query_deserialize_missing_response_type_fails() {
    let json = serde_json::json!({
        "client_id": "sp_test_client"
    });

    let result = serde_json::from_value::<AuthorizeQuery>(json);
    assert!(result.is_err());
}

#[test]
fn test_authorize_query_deserialize_missing_client_id_fails() {
    let json = serde_json::json!({
        "response_type": "code"
    });

    let result = serde_json::from_value::<AuthorizeQuery>(json);
    assert!(result.is_err());
}

#[test]
fn test_authorize_query_deserialize_max_age_negative() {
    let json = serde_json::json!({
        "response_type": "code",
        "client_id": "sp_test_client",
        "max_age": -1
    });

    let query: AuthorizeQuery = serde_json::from_value(json).expect("should deserialize");
    assert_eq!(query.max_age, Some(-1));
}

#[test]
fn test_authorize_query_deserialize_max_age_zero() {
    let json = serde_json::json!({
        "response_type": "code",
        "client_id": "sp_test_client",
        "max_age": 0
    });

    let query: AuthorizeQuery = serde_json::from_value(json).expect("should deserialize");
    assert_eq!(query.max_age, Some(0));
}

#[test]
fn test_authorize_query_debug_trait() {
    let query = create_valid_authorize_query();
    let debug_output = format!("{:?}", query);

    assert!(debug_output.contains("AuthorizeQuery"));
    assert!(debug_output.contains("code"));
    assert!(debug_output.contains("sp_test_client"));
}

#[test]
fn test_authorize_query_field_access() {
    let query = create_valid_authorize_query();

    assert_eq!(query.response_type, "code");
    assert_eq!(query.client_id.as_str(), "sp_test_client");
    assert_eq!(query.redirect_uri.as_deref(), Some("https://example.com/callback"));
    assert_eq!(query.scope.as_deref(), Some("openid"));
    assert_eq!(query.state.as_deref(), Some("random_state_value"));
    assert!(query.code_challenge.is_some());
    assert_eq!(query.code_challenge_method.as_deref(), Some("S256"));
    assert!(query.response_mode.is_none());
    assert!(query.display.is_none());
    assert!(query.prompt.is_none());
    assert!(query.max_age.is_none());
    assert!(query.ui_locales.is_none());
    assert!(query.resource.is_none());
}

#[test]
fn test_authorize_query_client_id_types() {
    let first_party_query = AuthorizeQuery {
        client_id: ClientId::new("sp_web"),
        ..create_valid_authorize_query()
    };
    assert!(first_party_query.client_id.as_str().starts_with("sp_"));

    let cimd_query = AuthorizeQuery {
        client_id: ClientId::new("https://example.com/.well-known/oauth-client"),
        ..create_valid_authorize_query()
    };
    assert!(cimd_query.client_id.is_cimd());

    let system_query = AuthorizeQuery {
        client_id: ClientId::new("sys_scheduler"),
        ..create_valid_authorize_query()
    };
    assert!(system_query.client_id.is_system());
}

// ============================================================================
// AuthorizeRequest Deserialization
// ============================================================================

#[test]
fn test_authorize_request_deserialize_all_fields() {
    let json = serde_json::json!({
        "response_type": "code",
        "client_id": "sp_test_client",
        "redirect_uri": "https://example.com/callback",
        "scope": "openid profile",
        "state": "abc123",
        "code_challenge": "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
        "code_challenge_method": "S256",
        "user_consent": "allow",
        "username": "testuser",
        "password": "testpass",
        "resource": "https://api.example.com"
    });

    let request: AuthorizeRequest = serde_json::from_value(json).expect("should deserialize");

    assert_eq!(request.response_type, "code");
    assert_eq!(request.client_id.as_str(), "sp_test_client");
    assert_eq!(request.redirect_uri.as_deref(), Some("https://example.com/callback"));
    assert_eq!(request.scope.as_deref(), Some("openid profile"));
    assert_eq!(request.state.as_deref(), Some("abc123"));
    assert_eq!(
        request.code_challenge.as_deref(),
        Some("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk")
    );
    assert_eq!(request.code_challenge_method.as_deref(), Some("S256"));
    assert_eq!(request.user_consent.as_deref(), Some("allow"));
    assert_eq!(request.username.as_deref(), Some("testuser"));
    assert_eq!(request.password.as_deref(), Some("testpass"));
    assert_eq!(request.resource.as_deref(), Some("https://api.example.com"));
}

#[test]
fn test_authorize_request_deserialize_required_fields_only() {
    let json = serde_json::json!({
        "response_type": "code",
        "client_id": "sp_test_client"
    });

    let request: AuthorizeRequest = serde_json::from_value(json).expect("should deserialize");

    assert_eq!(request.response_type, "code");
    assert_eq!(request.client_id.as_str(), "sp_test_client");
    assert!(request.redirect_uri.is_none());
    assert!(request.scope.is_none());
    assert!(request.state.is_none());
    assert!(request.code_challenge.is_none());
    assert!(request.code_challenge_method.is_none());
    assert!(request.user_consent.is_none());
    assert!(request.username.is_none());
    assert!(request.password.is_none());
    assert!(request.resource.is_none());
}

#[test]
fn test_authorize_request_deserialize_missing_response_type_fails() {
    let json = serde_json::json!({
        "client_id": "sp_test_client"
    });

    let result = serde_json::from_value::<AuthorizeRequest>(json);
    assert!(result.is_err());
}

#[test]
fn test_authorize_request_deserialize_missing_client_id_fails() {
    let json = serde_json::json!({
        "response_type": "code"
    });

    let result = serde_json::from_value::<AuthorizeRequest>(json);
    assert!(result.is_err());
}

#[test]
fn test_authorize_request_consent_allow() {
    let request = create_valid_authorize_request();
    assert_eq!(request.user_consent.as_deref(), Some("allow"));
}

#[test]
fn test_authorize_request_consent_deny() {
    let request = AuthorizeRequest {
        user_consent: Some("deny".to_string()),
        ..create_valid_authorize_request()
    };
    assert_eq!(request.user_consent.as_deref(), Some("deny"));
}

#[test]
fn test_authorize_request_consent_none() {
    let request = AuthorizeRequest {
        user_consent: None,
        ..create_valid_authorize_request()
    };
    assert!(request.user_consent.is_none());
}

#[test]
fn test_authorize_request_debug_trait() {
    let request = create_valid_authorize_request();
    let debug_output = format!("{:?}", request);

    assert!(debug_output.contains("AuthorizeRequest"));
    assert!(debug_output.contains("code"));
    assert!(debug_output.contains("sp_test_client"));
}

// ============================================================================
// AuthorizeResponse Serialization
// ============================================================================

#[test]
fn test_authorize_response_serialize_success() {
    let response = AuthorizeResponse {
        code: Some("auth_code_123".to_string()),
        state: Some("abc123".to_string()),
        error: None,
        error_description: None,
    };

    let json = serde_json::to_value(&response).expect("should serialize");

    assert_eq!(json["code"], "auth_code_123");
    assert_eq!(json["state"], "abc123");
    assert!(json.get("error").is_none());
    assert!(json.get("error_description").is_none());
}

#[test]
fn test_authorize_response_serialize_error() {
    let response = AuthorizeResponse {
        code: None,
        state: Some("abc123".to_string()),
        error: Some("access_denied".to_string()),
        error_description: Some("User denied the request".to_string()),
    };

    let json = serde_json::to_value(&response).expect("should serialize");

    assert!(json.get("code").is_none());
    assert_eq!(json["state"], "abc123");
    assert_eq!(json["error"], "access_denied");
    assert_eq!(json["error_description"], "User denied the request");
}

#[test]
fn test_authorize_response_serialize_all_none() {
    let response = AuthorizeResponse {
        code: None,
        state: None,
        error: None,
        error_description: None,
    };

    let json = serde_json::to_value(&response).expect("should serialize");
    let obj = json.as_object().expect("should be object");

    assert!(obj.is_empty());
}

#[test]
fn test_authorize_response_serialize_all_present() {
    let response = AuthorizeResponse {
        code: Some("auth_code_123".to_string()),
        state: Some("abc123".to_string()),
        error: Some("server_error".to_string()),
        error_description: Some("Something went wrong".to_string()),
    };

    let json = serde_json::to_value(&response).expect("should serialize");
    let obj = json.as_object().expect("should be object");

    assert_eq!(obj.len(), 4);
    assert_eq!(json["code"], "auth_code_123");
    assert_eq!(json["state"], "abc123");
    assert_eq!(json["error"], "server_error");
    assert_eq!(json["error_description"], "Something went wrong");
}

#[test]
fn test_authorize_response_skip_serializing_none_fields() {
    let response = AuthorizeResponse {
        code: Some("auth_code_123".to_string()),
        state: None,
        error: None,
        error_description: None,
    };

    let json_string = serde_json::to_string(&response).expect("should serialize");

    assert!(json_string.contains("code"));
    assert!(!json_string.contains("state"));
    assert!(!json_string.contains("error"));
    assert!(!json_string.contains("error_description"));
}

#[test]
fn test_authorize_response_debug_trait() {
    let response = AuthorizeResponse {
        code: Some("auth_code_123".to_string()),
        state: Some("abc123".to_string()),
        error: None,
        error_description: None,
    };

    let debug_output = format!("{:?}", response);
    assert!(debug_output.contains("AuthorizeResponse"));
    assert!(debug_output.contains("auth_code_123"));
}

// ============================================================================
// AuthorizeQuery and AuthorizeRequest Field Correspondence
// ============================================================================

#[test]
fn test_query_and_request_share_common_fields() {
    let query = create_valid_authorize_query();
    let request = create_valid_authorize_request();

    assert_eq!(query.response_type, request.response_type);
    assert_eq!(query.client_id, request.client_id);
    assert_eq!(query.redirect_uri, request.redirect_uri);
    assert_eq!(query.scope, request.scope);
    assert_eq!(query.state, request.state);
    assert_eq!(query.code_challenge, request.code_challenge);
    assert_eq!(query.code_challenge_method, request.code_challenge_method);
}

#[test]
fn test_request_has_form_specific_fields() {
    let request = create_valid_authorize_request();

    assert!(request.user_consent.is_some());
    assert!(request.username.is_some());
    assert!(request.password.is_some());
}

#[test]
fn test_authorize_query_various_response_types_accepted_by_serde() {
    for response_type in &["code", "token", "id_token", ""] {
        let json = serde_json::json!({
            "response_type": response_type,
            "client_id": "sp_test_client"
        });

        let query: AuthorizeQuery = serde_json::from_value(json).expect("should deserialize");
        assert_eq!(query.response_type, *response_type);
    }
}

#[test]
fn test_authorize_query_various_display_values_accepted_by_serde() {
    for display in &["page", "popup", "touch", "wap", "fullscreen", "custom"] {
        let json = serde_json::json!({
            "response_type": "code",
            "client_id": "sp_test_client",
            "display": display
        });

        let query: AuthorizeQuery = serde_json::from_value(json).expect("should deserialize");
        assert_eq!(query.display.as_deref(), Some(*display));
    }
}

#[test]
fn test_authorize_response_error_types() {
    let error_types = vec![
        "invalid_request",
        "unauthorized_client",
        "access_denied",
        "unsupported_response_type",
        "invalid_scope",
        "server_error",
        "temporarily_unavailable",
    ];

    for error_type in &error_types {
        let response = AuthorizeResponse {
            code: None,
            state: Some("state".to_string()),
            error: Some(error_type.to_string()),
            error_description: Some(format!("Error: {error_type}")),
        };

        let json = serde_json::to_value(&response).expect("should serialize");
        assert_eq!(json["error"], *error_type);
    }
}

#[test]
fn test_authorize_query_with_resource_uri() {
    let query = AuthorizeQuery {
        resource: Some("https://api.example.com/v1".to_string()),
        ..create_valid_authorize_query()
    };

    assert_eq!(query.resource.as_deref(), Some("https://api.example.com/v1"));
}

#[test]
fn test_authorize_request_with_resource_uri() {
    let request = AuthorizeRequest {
        resource: Some("https://api.example.com/v1".to_string()),
        ..create_valid_authorize_request()
    };

    assert_eq!(request.resource.as_deref(), Some("https://api.example.com/v1"));
}
