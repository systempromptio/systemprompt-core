use axum::body::to_bytes;
use axum::http::StatusCode;
use systemprompt_api::routes::oauth::endpoints::token::generation::{
    convert_token_result_to_response, resolve_user_permissions,
};
use systemprompt_api::routes::oauth::endpoints::token::{
    TokenError, TokenResponse,
};
use systemprompt_models::auth::Permission;

#[test]
fn test_resolve_user_permissions_user_expands_to_user_roles() {
    let requested = vec![Permission::User];
    let user_perms = vec![Permission::Admin, Permission::User, Permission::A2a];
    let result = resolve_user_permissions(&requested, &user_perms).unwrap();
    assert!(result.contains(&Permission::Admin));
    assert!(result.contains(&Permission::User));
    assert!(!result.contains(&Permission::A2a));
}

#[test]
fn test_resolve_user_permissions_specific_permission_matched() {
    let requested = vec![Permission::Admin];
    let user_perms = vec![Permission::Admin, Permission::User];
    let result = resolve_user_permissions(&requested, &user_perms).unwrap();
    assert!(result.contains(&Permission::Admin));
    assert!(!result.contains(&Permission::User));
}

#[test]
fn test_resolve_user_permissions_unmatched_permission_excluded() {
    let requested = vec![Permission::Admin];
    let user_perms = vec![Permission::User, Permission::Anonymous];
    let result = resolve_user_permissions(&requested, &user_perms);
    assert!(result.is_err());
}

#[test]
fn test_resolve_user_permissions_empty_requested_returns_error() {
    let requested: Vec<Permission> = vec![];
    let user_perms = vec![Permission::Admin, Permission::User];
    let result = resolve_user_permissions(&requested, &user_perms);
    assert!(result.is_err());
}

#[test]
fn test_resolve_user_permissions_deduplicates() {
    let requested = vec![Permission::User, Permission::Admin];
    let user_perms = vec![Permission::Admin, Permission::User];
    let result = resolve_user_permissions(&requested, &user_perms).unwrap();
    let admin_count = result.iter().filter(|p| **p == Permission::Admin).count();
    assert_eq!(admin_count, 1);
}

#[test]
fn test_resolve_user_permissions_sorted_by_hierarchy() {
    let requested = vec![Permission::User, Permission::Admin];
    let user_perms = vec![Permission::Admin, Permission::User, Permission::Anonymous];
    let result = resolve_user_permissions(&requested, &user_perms).unwrap();
    assert!(result[0].hierarchy_level() >= result[result.len() - 1].hierarchy_level());
}

#[tokio::test]
async fn test_convert_token_result_ok_returns_200() {
    let token_response = TokenResponse {
        access_token: "at_test123".to_string(),
        token_type: "Bearer".to_string(),
        expires_in: 3600,
        refresh_token: Some("rt_test456".to_string()),
        scope: Some("user".to_string()),
    };
    let response = convert_token_result_to_response(Ok(token_response));
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_convert_token_result_ok_body_contains_token() {
    let token_response = TokenResponse {
        access_token: "at_test_token".to_string(),
        token_type: "Bearer".to_string(),
        expires_in: 7200,
        refresh_token: None,
        scope: None,
    };
    let response = convert_token_result_to_response(Ok(token_response));
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["access_token"], "at_test_token");
    assert_eq!(json["token_type"], "Bearer");
    assert_eq!(json["expires_in"], 7200);
}

#[tokio::test]
async fn test_convert_token_result_invalid_client_secret_returns_401() {
    let result = Err(TokenError::InvalidClientSecret);
    let response = convert_token_result_to_response(result);
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_convert_token_result_server_error_returns_500() {
    let result = Err(TokenError::ServerError {
        message: "db timeout".to_string(),
    });
    let response = convert_token_result_to_response(result);
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_convert_token_result_invalid_request_returns_400() {
    let result = Err(TokenError::InvalidRequest {
        field: "code".to_string(),
        message: "is required".to_string(),
    });
    let response = convert_token_result_to_response(result);
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_convert_token_result_invalid_grant_returns_400() {
    let result = Err(TokenError::InvalidGrant {
        reason: "code already used".to_string(),
    });
    let response = convert_token_result_to_response(result);
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_convert_token_result_expired_code_returns_400() {
    let result = Err(TokenError::ExpiredCode);
    let response = convert_token_result_to_response(result);
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_convert_token_result_error_body_contains_error_type() {
    let result = Err(TokenError::InvalidClient);
    let response = convert_token_result_to_response(result);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "invalid_client");
    assert!(json["error_description"].is_string());
}
