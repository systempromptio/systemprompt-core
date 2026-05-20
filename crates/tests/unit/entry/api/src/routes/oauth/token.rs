use axum::body::to_bytes;
use axum::response::IntoResponse;
use http::StatusCode;
use systemprompt_api::routes::oauth::OAuthHttpError;
use systemprompt_api::routes::oauth::discovery::{
    OAuthProtectedResourceResponse, WellKnownResponse,
};
use systemprompt_api::routes::oauth::endpoints::token::{TokenError, TokenRequest, TokenResponse};

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

async fn body_to_json(resp: axum::response::Response) -> serde_json::Value {
    let body = to_bytes(resp.into_body(), 65_536).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn token_error_invalid_request_maps_to_invalid_request_wire_code() {
    let http: OAuthHttpError = TokenError::InvalidRequest {
        field: "code".to_string(),
        message: "missing".to_string(),
    }
    .into();
    let resp = http.into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "invalid_request");
    let desc = json["error_description"].as_str().unwrap();
    assert!(desc.contains("code"));
    assert!(desc.contains("missing"));
}

#[tokio::test]
async fn token_error_unsupported_grant_maps_to_unsupported_grant_type() {
    let http: OAuthHttpError = TokenError::UnsupportedGrantType {
        grant_type: "device_code".to_string(),
    }
    .into();
    let resp = http.into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "unsupported_grant_type");
    assert!(json["error_description"].as_str().unwrap().contains("device_code"));
}

#[tokio::test]
async fn token_error_invalid_client_maps_to_invalid_client_with_401() {
    let resp = OAuthHttpError::from(TokenError::InvalidClient).into_response();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "invalid_client");
}

#[tokio::test]
async fn token_error_invalid_grant_maps_to_invalid_grant() {
    let resp = OAuthHttpError::from(TokenError::InvalidGrant {
        reason: "code mismatch".to_string(),
    })
    .into_response();
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "invalid_grant");
    assert_eq!(json["error_description"], "code mismatch");
}

#[tokio::test]
async fn token_error_expired_code_maps_to_invalid_grant() {
    let resp = OAuthHttpError::from(TokenError::ExpiredCode).into_response();
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "invalid_grant");
    assert!(json["error_description"].as_str().unwrap().contains("expired"));
}

#[tokio::test]
async fn token_error_server_error_maps_to_server_error_with_500() {
    let resp = OAuthHttpError::from(TokenError::ServerError {
        message: "timeout".to_string(),
    })
    .into_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "server_error");
    assert_eq!(json["error_description"], "timeout");
}

#[tokio::test]
async fn token_error_invalid_refresh_token_maps_to_invalid_grant() {
    let resp = OAuthHttpError::from(TokenError::InvalidRefreshToken {
        reason: "token revoked".to_string(),
    })
    .into_response();
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "invalid_grant");
    assert!(json["error_description"].as_str().unwrap().contains("token revoked"));
}

#[tokio::test]
async fn token_error_invalid_credentials_maps_to_invalid_grant() {
    let resp = OAuthHttpError::from(TokenError::InvalidCredentials).into_response();
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "invalid_grant");
}

#[tokio::test]
async fn token_error_invalid_client_secret_maps_to_invalid_client() {
    let resp = OAuthHttpError::from(TokenError::InvalidClientSecret).into_response();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "invalid_client");
    assert!(json["error_description"].as_str().unwrap().contains("client secret"));
}

#[tokio::test]
async fn token_error_invalid_target_maps_to_invalid_target() {
    let resp = OAuthHttpError::from(TokenError::InvalidTarget {
        message: "unknown resource".to_string(),
    })
    .into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "invalid_target");
}

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

#[test]
fn test_token_response_serialize_full() {
    let response = TokenResponse {
        access_token: "at_xyz".to_string(),
        token_type: "Bearer".to_string(),
        expires_in: 3600,
        refresh_token: Some("rt_abc".to_string()),
        scope: Some("openid profile".to_string()),
        issued_token_type: None,
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
        issued_token_type: None,
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
        issued_token_type: None,
    };

    let debug = format!("{response:?}");
    assert!(debug.contains("TokenResponse"));
    assert!(debug.contains("at_test"));
}

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
