//! Tests for OAuth discovery/wellknown response types and the OAuthHttpError
//! wire shape.

use axum::body::to_bytes;
use axum::response::IntoResponse;
use http::StatusCode;
use systemprompt_api::routes::oauth::OAuthHttpError;
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
        authorization_response_iss_parameter_supported: true,
    };

    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["issuer"], "https://example.com");
    assert_eq!(
        json["authorization_endpoint"],
        "https://example.com/authorize"
    );
    assert_eq!(json["token_endpoint"], "https://example.com/token");
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
        authorization_response_iss_parameter_supported: true,
    };

    let json = serde_json::to_value(&response).unwrap();
    assert!(json["registration_endpoint"].is_null());
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
    assert_eq!(json["authorization_servers"][0], "https://auth.example.com");
}

async fn body_to_json(resp: axum::response::Response) -> serde_json::Value {
    let body = to_bytes(resp.into_body(), 65_536).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn oauth_http_error_invalid_request_wire_shape() {
    let resp = OAuthHttpError::invalid_request("Missing client_id").into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "invalid_request");
    assert_eq!(json["error_description"], "Missing client_id");
}

#[tokio::test]
async fn oauth_http_error_invalid_client_sets_www_authenticate() {
    let resp = OAuthHttpError::invalid_client("bad creds").into_response();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let www = resp
        .headers()
        .get(http::header::WWW_AUTHENTICATE)
        .expect("WWW-Authenticate present on 401");
    assert!(www.to_str().unwrap().starts_with("Bearer "));
}

#[tokio::test]
async fn oauth_http_error_server_error_status() {
    let resp = OAuthHttpError::server_error("boom").into_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "server_error");
}

#[tokio::test]
async fn oauth_http_error_redirect_path() {
    let resp = OAuthHttpError::invalid_request("Validation failed")
        .with_redirect("https://client.example/cb", Some("xyz".to_string()))
        .into_response();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let location = resp
        .headers()
        .get(http::header::LOCATION)
        .expect("Location header on redirect");
    let loc = location.to_str().unwrap();
    assert!(loc.starts_with("https://client.example/cb?"));
    assert!(loc.contains("error=invalid_request"));
    assert!(loc.contains("error_description=Validation%20failed"));
    assert!(loc.contains("state=xyz"));
}

#[tokio::test]
async fn oauth_http_error_username_unavailable_maps_to_conflict() {
    let resp = OAuthHttpError::username_unavailable("taken").into_response();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "username_unavailable");
}
