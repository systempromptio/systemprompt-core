use axum::body::to_bytes;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use systemprompt_api::routes::oauth::OAuthHttpError;
use systemprompt_oauth_issuance::IssuanceError;

async fn body_to_json(resp: axum::response::Response) -> serde_json::Value {
    let body = to_bytes(resp.into_body(), 65_536).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn token_error_invalid_request_maps_to_invalid_request_wire_code() {
    let http: OAuthHttpError = IssuanceError::InvalidRequest {
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
    let http: OAuthHttpError = IssuanceError::UnsupportedGrantType {
        grant_type: "device_code".to_string(),
    }
    .into();
    let resp = http.into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "unsupported_grant_type");
    assert!(
        json["error_description"]
            .as_str()
            .unwrap()
            .contains("device_code")
    );
}

#[tokio::test]
async fn token_error_invalid_client_maps_to_invalid_client_with_401() {
    let resp = OAuthHttpError::from(IssuanceError::InvalidClient).into_response();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "invalid_client");
}

#[tokio::test]
async fn token_error_invalid_grant_maps_to_invalid_grant() {
    let resp = OAuthHttpError::from(IssuanceError::InvalidGrant {
        reason: "code mismatch".to_string(),
    })
    .into_response();
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "invalid_grant");
    assert_eq!(json["error_description"], "code mismatch");
}

#[tokio::test]
async fn token_error_expired_code_maps_to_invalid_grant() {
    let resp = OAuthHttpError::from(IssuanceError::ExpiredCode).into_response();
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "invalid_grant");
    assert!(
        json["error_description"]
            .as_str()
            .unwrap()
            .contains("expired")
    );
}

#[tokio::test]
async fn token_error_server_error_maps_to_server_error_with_500() {
    let resp = OAuthHttpError::from(IssuanceError::server(
        "Token generation failed",
        std::io::Error::other("timeout"),
    ))
    .into_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "server_error");
    let description = json["error_description"].as_str().unwrap_or_default();
    assert!(!description.contains("timeout"), "{description}");
}

#[tokio::test]
async fn token_error_invalid_refresh_token_maps_to_invalid_grant() {
    let resp = OAuthHttpError::from(IssuanceError::InvalidRefreshToken {
        reason: "token revoked".to_string(),
    })
    .into_response();
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "invalid_grant");
    assert!(
        json["error_description"]
            .as_str()
            .unwrap()
            .contains("token revoked")
    );
}

#[tokio::test]
async fn token_error_invalid_credentials_maps_to_invalid_grant() {
    let resp = OAuthHttpError::from(IssuanceError::InvalidCredentials).into_response();
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "invalid_grant");
}

#[tokio::test]
async fn token_error_invalid_client_secret_maps_to_invalid_client() {
    let resp = OAuthHttpError::from(IssuanceError::InvalidClientSecret).into_response();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "invalid_client");
    assert!(
        json["error_description"]
            .as_str()
            .unwrap()
            .contains("client secret")
    );
}

#[tokio::test]
async fn token_error_invalid_target_maps_to_invalid_target() {
    let resp = OAuthHttpError::from(IssuanceError::InvalidTarget {
        message: "unknown resource".to_string(),
    })
    .into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_to_json(resp).await;
    assert_eq!(json["error"], "invalid_target");
}

#[tokio::test]
async fn token_error_invalid_client_secret_yields_401() {
    let resp = OAuthHttpError::from(IssuanceError::InvalidClientSecret).into_response();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn token_error_server_error_yields_500() {
    let resp = OAuthHttpError::from(IssuanceError::server(
        "Token generation failed",
        std::io::Error::other("db timeout"),
    ))
    .into_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn token_error_invalid_request_yields_400() {
    let resp = OAuthHttpError::from(IssuanceError::InvalidRequest {
        field: "code".to_string(),
        message: "is required".to_string(),
    })
    .into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn token_error_invalid_grant_yields_400() {
    let resp = OAuthHttpError::from(IssuanceError::InvalidGrant {
        reason: "code already used".to_string(),
    })
    .into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn token_error_expired_code_yields_400() {
    let resp = OAuthHttpError::from(IssuanceError::ExpiredCode).into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn token_error_invalid_client_body_has_error_code() {
    let resp = OAuthHttpError::from(IssuanceError::InvalidClient).into_response();
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "invalid_client");
    assert!(json["error_description"].is_string());
}
