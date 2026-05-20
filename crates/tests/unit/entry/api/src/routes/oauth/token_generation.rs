use axum::body::to_bytes;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use systemprompt_api::routes::oauth::OAuthHttpError;
use systemprompt_api::routes::oauth::endpoints::token::TokenError;
use systemprompt_api::routes::oauth::endpoints::token::generation::resolve_user_permissions;
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
async fn token_error_invalid_client_secret_yields_401() {
    let resp = OAuthHttpError::from(TokenError::InvalidClientSecret).into_response();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn token_error_server_error_yields_500() {
    let resp = OAuthHttpError::from(TokenError::ServerError {
        message: "db timeout".to_string(),
    })
    .into_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn token_error_invalid_request_yields_400() {
    let resp = OAuthHttpError::from(TokenError::InvalidRequest {
        field: "code".to_string(),
        message: "is required".to_string(),
    })
    .into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn token_error_invalid_grant_yields_400() {
    let resp = OAuthHttpError::from(TokenError::InvalidGrant {
        reason: "code already used".to_string(),
    })
    .into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn token_error_expired_code_yields_400() {
    let resp = OAuthHttpError::from(TokenError::ExpiredCode).into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn token_error_invalid_client_body_has_error_code() {
    let resp = OAuthHttpError::from(TokenError::InvalidClient).into_response();
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "invalid_client");
    assert!(json["error_description"].is_string());
}
