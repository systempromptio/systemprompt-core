use systemprompt_api::routes::oauth::discovery::{
    OAuthProtectedResourceResponse, WellKnownResponse,
};
use systemprompt_api::routes::oauth::endpoints::token::{
    TokenError, TokenErrorResponse, TokenRequest, TokenResponse,
};

// ============================================================================
// TokenError Display Tests
// ============================================================================

#[test]
fn test_token_error_invalid_request_display() {
    let error = TokenError::InvalidRequest {
        field: "redirect_uri".to_string(),
        message: "is required".to_string(),
    };

    let display = format!("{error}");
    assert!(display.contains("redirect_uri"));
    assert!(display.contains("is required"));
}

#[test]
fn test_token_error_unsupported_grant_type_display() {
    let error = TokenError::UnsupportedGrantType {
        grant_type: "implicit".to_string(),
    };

    let display = format!("{error}");
    assert!(display.contains("implicit"));
}

#[test]
fn test_token_error_invalid_client_display() {
    let error = TokenError::InvalidClient;

    let display = format!("{error}");
    assert!(display.contains("Invalid client credentials"));
}

#[test]
fn test_token_error_invalid_grant_display() {
    let error = TokenError::InvalidGrant {
        reason: "code already used".to_string(),
    };

    let display = format!("{error}");
    assert!(display.contains("code already used"));
}

#[test]
fn test_token_error_expired_code_display() {
    let error = TokenError::ExpiredCode;

    let display = format!("{error}");
    assert!(display.contains("expired"));
}

#[test]
fn test_token_error_server_error_display() {
    let error = TokenError::ServerError {
        message: "database unavailable".to_string(),
    };

    let display = format!("{error}");
    assert!(display.contains("database unavailable"));
}

// ============================================================================
// TokenError -> TokenErrorResponse Conversion Tests
// ============================================================================

#[test]
fn test_token_error_response_from_invalid_request() {
    let error = TokenError::InvalidRequest {
        field: "code".to_string(),
        message: "missing".to_string(),
    };
    let response: TokenErrorResponse = error.into();

    assert_eq!(response.error, "invalid_request");
    assert!(response.error_description.is_some());
    let desc = response.error_description.unwrap();
    assert!(desc.contains("code"));
    assert!(desc.contains("missing"));
}

#[test]
fn test_token_error_response_from_unsupported_grant() {
    let error = TokenError::UnsupportedGrantType {
        grant_type: "device_code".to_string(),
    };
    let response: TokenErrorResponse = error.into();

    assert_eq!(response.error, "unsupported_grant_type");
    assert!(
        response
            .error_description
            .as_deref()
            .unwrap()
            .contains("device_code")
    );
}

#[test]
fn test_token_error_response_from_invalid_client() {
    let error = TokenError::InvalidClient;
    let response: TokenErrorResponse = error.into();

    assert_eq!(response.error, "invalid_client");
    assert!(response.error_description.is_some());
}

#[test]
fn test_token_error_response_from_invalid_grant() {
    let error = TokenError::InvalidGrant {
        reason: "code mismatch".to_string(),
    };
    let response: TokenErrorResponse = error.into();

    assert_eq!(response.error, "invalid_grant");
    assert_eq!(response.error_description.as_deref(), Some("code mismatch"));
}

#[test]
fn test_token_error_response_from_expired_code() {
    let error = TokenError::ExpiredCode;
    let response: TokenErrorResponse = error.into();

    assert_eq!(response.error, "invalid_grant");
    assert!(
        response
            .error_description
            .as_deref()
            .unwrap()
            .contains("expired")
    );
}

#[test]
fn test_token_error_response_from_server_error() {
    let error = TokenError::ServerError {
        message: "timeout".to_string(),
    };
    let response: TokenErrorResponse = error.into();

    assert_eq!(response.error, "server_error");
    assert_eq!(response.error_description.as_deref(), Some("timeout"));
}

#[test]
fn test_token_error_response_from_invalid_refresh_token() {
    let error = TokenError::InvalidRefreshToken {
        reason: "token revoked".to_string(),
    };
    let response: TokenErrorResponse = error.into();

    assert_eq!(response.error, "invalid_grant");
    assert!(
        response
            .error_description
            .as_deref()
            .unwrap()
            .contains("token revoked")
    );
}

#[test]
fn test_token_error_response_from_invalid_credentials() {
    let error = TokenError::InvalidCredentials;
    let response: TokenErrorResponse = error.into();

    assert_eq!(response.error, "invalid_grant");
    assert!(
        response
            .error_description
            .as_deref()
            .unwrap()
            .contains("Invalid credentials")
    );
}

#[test]
fn test_token_error_response_from_invalid_client_secret() {
    let error = TokenError::InvalidClientSecret;
    let response: TokenErrorResponse = error.into();

    assert_eq!(response.error, "invalid_client");
    assert!(
        response
            .error_description
            .as_deref()
            .unwrap()
            .contains("client secret")
    );
}

// ============================================================================
// TokenRequest Deserialization Tests
// ============================================================================

#[test]
fn test_token_request_deserialize_authorization_code() {
    let json = serde_json::json!({
        "grant_type": "authorization_code",
        "code": "abc123",
        "redirect_uri": "https://example.com/callback",
        "client_id": "client-1",
        "client_secret": "secret-1",
        "scope": "openid profile",
        "code_verifier": "verifier-value",
        "resource": "https://api.example.com"
    });

    let request: TokenRequest = serde_json::from_value(json).unwrap();

    assert_eq!(request.grant_type, "authorization_code");
    assert_eq!(request.code.as_deref(), Some("abc123"));
    assert_eq!(
        request.redirect_uri.as_deref(),
        Some("https://example.com/callback")
    );
    assert_eq!(request.client_id.as_deref(), Some("client-1"));
    assert_eq!(request.client_secret.as_deref(), Some("secret-1"));
    assert_eq!(request.scope.as_deref(), Some("openid profile"));
    assert_eq!(request.code_verifier.as_deref(), Some("verifier-value"));
    assert_eq!(request.resource.as_deref(), Some("https://api.example.com"));
    assert!(request.refresh_token.is_none());
}

#[test]
fn test_token_request_deserialize_minimal() {
    let json = serde_json::json!({
        "grant_type": "client_credentials"
    });

    let request: TokenRequest = serde_json::from_value(json).unwrap();

    assert_eq!(request.grant_type, "client_credentials");
    assert!(request.code.is_none());
    assert!(request.redirect_uri.is_none());
    assert!(request.client_id.is_none());
    assert!(request.client_secret.is_none());
    assert!(request.refresh_token.is_none());
    assert!(request.scope.is_none());
    assert!(request.code_verifier.is_none());
    assert!(request.resource.is_none());
}

#[test]
fn test_token_request_debug() {
    let json = serde_json::json!({
        "grant_type": "refresh_token",
        "refresh_token": "rt_abc123"
    });

    let request: TokenRequest = serde_json::from_value(json).unwrap();
    let debug = format!("{request:?}");

    assert!(debug.contains("TokenRequest"));
    assert!(debug.contains("refresh_token"));
}

// ============================================================================
// TokenResponse Serialization Tests
// ============================================================================

#[test]
fn test_token_response_serialize_full() {
    let response = TokenResponse {
        access_token: "at_xyz".to_string(),
        token_type: "Bearer".to_string(),
        expires_in: 3600,
        refresh_token: Some("rt_abc".to_string()),
        scope: Some("openid profile".to_string()),
    };

    let json = serde_json::to_value(&response).unwrap();

    assert_eq!(json["access_token"], "at_xyz");
    assert_eq!(json["token_type"], "Bearer");
    assert_eq!(json["expires_in"], 3600);
    assert_eq!(json["refresh_token"], "rt_abc");
    assert_eq!(json["scope"], "openid profile");
}

#[test]
fn test_token_response_serialize_skip_none() {
    let response = TokenResponse {
        access_token: "at_xyz".to_string(),
        token_type: "Bearer".to_string(),
        expires_in: 7200,
        refresh_token: None,
        scope: None,
    };

    let json = serde_json::to_value(&response).unwrap();

    assert_eq!(json["access_token"], "at_xyz");
    assert_eq!(json["expires_in"], 7200);
    assert!(json.get("refresh_token").is_none());
    assert!(json.get("scope").is_none());
}

#[test]
fn test_token_response_debug() {
    let response = TokenResponse {
        access_token: "at_test".to_string(),
        token_type: "Bearer".to_string(),
        expires_in: 3600,
        refresh_token: None,
        scope: None,
    };

    let debug = format!("{response:?}");
    assert!(debug.contains("TokenResponse"));
    assert!(debug.contains("at_test"));
}

// ============================================================================
// TokenErrorResponse Serialization Tests
// ============================================================================

#[test]
fn test_token_error_response_serialize() {
    let response = TokenErrorResponse {
        error: "invalid_request".to_string(),
        error_description: Some("Missing code parameter".to_string()),
    };

    let json = serde_json::to_value(&response).unwrap();

    assert_eq!(json["error"], "invalid_request");
    assert_eq!(json["error_description"], "Missing code parameter");
}

#[test]
fn test_token_error_response_skip_none_description() {
    let response = TokenErrorResponse {
        error: "server_error".to_string(),
        error_description: None,
    };

    let json = serde_json::to_value(&response).unwrap();

    assert_eq!(json["error"], "server_error");
    assert!(json.get("error_description").is_none());
}

// ============================================================================
// WellKnownResponse Serialization Tests
// ============================================================================

#[test]
fn test_well_known_response_serialize() {
    let response = WellKnownResponse {
        issuer: "https://auth.example.com".to_string(),
        authorization_endpoint: "https://auth.example.com/authorize".to_string(),
        token_endpoint: "https://auth.example.com/token".to_string(),
        userinfo_endpoint: "https://auth.example.com/userinfo".to_string(),
        introspection_endpoint: "https://auth.example.com/introspect".to_string(),
        revocation_endpoint: "https://auth.example.com/revoke".to_string(),
        registration_endpoint: Some("https://auth.example.com/register".to_string()),
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

    assert_eq!(json["issuer"], "https://auth.example.com");
    assert_eq!(
        json["authorization_endpoint"],
        "https://auth.example.com/authorize"
    );
    assert_eq!(json["token_endpoint"], "https://auth.example.com/token");
    assert_eq!(
        json["registration_endpoint"],
        "https://auth.example.com/register"
    );
    assert_eq!(json["scopes_supported"].as_array().unwrap().len(), 2);
    assert_eq!(
        json["code_challenge_methods_supported"].as_array().unwrap()[0],
        "S256"
    );
}

// ============================================================================
// OAuthProtectedResourceResponse Serialization Tests
// ============================================================================

#[test]
fn test_oauth_protected_resource_response_serialize() {
    let response = OAuthProtectedResourceResponse {
        resource: "https://api.example.com".to_string(),
        authorization_servers: vec!["https://auth.example.com".to_string()],
        scopes_supported: vec!["read".to_string(), "write".to_string()],
        bearer_methods_supported: vec!["header".to_string()],
        resource_documentation: Some("https://docs.example.com".to_string()),
    };

    let json = serde_json::to_value(&response).unwrap();

    assert_eq!(json["resource"], "https://api.example.com");
    assert_eq!(json["authorization_servers"].as_array().unwrap().len(), 1);
    assert_eq!(json["scopes_supported"].as_array().unwrap().len(), 2);
    assert_eq!(json["bearer_methods_supported"][0], "header");
    assert_eq!(json["resource_documentation"], "https://docs.example.com");
}

#[test]
fn test_oauth_protected_resource_response_debug() {
    let response = OAuthProtectedResourceResponse {
        resource: "https://api.example.com".to_string(),
        authorization_servers: vec![],
        scopes_supported: vec![],
        bearer_methods_supported: vec![],
        resource_documentation: None,
    };

    let debug = format!("{response:?}");
    assert!(debug.contains("OAuthProtectedResourceResponse"));
}
