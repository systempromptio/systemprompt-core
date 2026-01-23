//! Integration tests for OAuth authorization codes and refresh tokens

use crate::{cleanup_test_user, create_test_user, setup_test_db};
use chrono::Utc;
use systemprompt_identifiers::{AuthorizationCode, ClientId, RefreshTokenId};
use systemprompt_oauth::repository::{
    AuthCodeParams, ClientRepository, CreateClientParams, OAuthRepository, RefreshTokenParams,
};
use uuid::Uuid;

fn test_code() -> AuthorizationCode {
    AuthorizationCode::new(&format!("code_{}", Uuid::new_v4()))
}

fn test_client_id() -> ClientId {
    ClientId::new(&format!("test_client_{}", Uuid::new_v4()))
}

fn test_token_id() -> RefreshTokenId {
    RefreshTokenId::new(&format!("token_{}", Uuid::new_v4()))
}

async fn create_test_client(
    db: &systemprompt_database::DbPool,
    client_id: &ClientId,
) -> ClientId {
    let repo = ClientRepository::new(db).expect("Failed to create client repo");
    let params = CreateClientParams {
        client_id: client_id.clone(),
        client_secret_hash: "test_hash".to_string(),
        client_name: "Test Client".to_string(),
        redirect_uris: vec!["http://localhost:3000/callback".to_string()],
        grant_types: None,
        response_types: None,
        scopes: vec!["openid".to_string()],
        token_endpoint_auth_method: None,
        client_uri: None,
        logo_uri: None,
        contacts: None,
    };
    repo.create(params).await.expect("Failed to create test client");
    client_id.clone()
}

async fn cleanup_test_client(db: &systemprompt_database::DbPool, client_id: &ClientId) {
    let repo = ClientRepository::new(db).expect("Failed to create client repo");
    let _ = repo.delete(client_id.as_str()).await;
}

#[tokio::test]
async fn test_authorization_code_lifecycle() {
    let db = setup_test_db().await;
    let client_id = test_client_id();
    create_test_client(&db, &client_id).await;
    let user_id = create_test_user(&db).await;

    let repo = OAuthRepository::new(db.clone()).expect("Failed to create repository");

    let code = test_code();
    let redirect_uri = "http://localhost:3000/callback";
    let scopes = "openid profile";

    let params = AuthCodeParams::builder(&code, &client_id, &user_id, redirect_uri, scopes).build();
    repo.store_authorization_code(params)
        .await
        .expect("Failed to store authorization code");

    let stored_client_id = repo
        .get_client_id_from_auth_code(&code)
        .await
        .expect("Failed to get client_id from code")
        .expect("Code not found");

    assert_eq!(stored_client_id.as_str(), client_id.as_str());

    let (returned_user_id, returned_scope) = repo
        .validate_authorization_code(&code, &client_id, Some(redirect_uri), None)
        .await
        .expect("Failed to validate authorization code");

    assert_eq!(returned_user_id.as_str(), user_id.as_str());
    assert_eq!(returned_scope, scopes);

    let validation_again = repo
        .validate_authorization_code(&code, &client_id, Some(redirect_uri), None)
        .await;

    assert!(
        validation_again.is_err(),
        "Should not be able to use code twice"
    );

    cleanup_test_client(&db, &client_id).await;
    cleanup_test_user(&db, &user_id).await;
}

#[tokio::test]
async fn test_authorization_code_pkce() {
    let db = setup_test_db().await;
    let client_id = test_client_id();
    create_test_client(&db, &client_id).await;
    let user_id = create_test_user(&db).await;

    let repo = OAuthRepository::new(db.clone()).expect("Failed to create repository");

    let code = test_code();
    let redirect_uri = "http://localhost:3000/callback";

    use base64::Engine;
    use sha2::{Digest, Sha256};
    let verifier = "test_verifier_string_that_is_long_enough_for_pkce";
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize());

    let params = AuthCodeParams::builder(&code, &client_id, &user_id, redirect_uri, "openid")
        .with_pkce(&challenge, "S256")
        .build();
    repo.store_authorization_code(params)
        .await
        .expect("Failed to store PKCE code");

    let validation = repo
        .validate_authorization_code(&code, &client_id, Some(redirect_uri), Some(verifier))
        .await
        .expect("Failed to validate PKCE code");

    assert_eq!(validation.0.as_str(), user_id.as_str());

    cleanup_test_client(&db, &client_id).await;
    cleanup_test_user(&db, &user_id).await;
}

#[tokio::test]
async fn test_authorization_code_pkce_invalid_verifier() {
    let db = setup_test_db().await;
    let client_id = test_client_id();
    create_test_client(&db, &client_id).await;
    let user_id = create_test_user(&db).await;

    let repo = OAuthRepository::new(db.clone()).expect("Failed to create repository");

    let code = test_code();
    let redirect_uri = "http://localhost:3000/callback";

    use base64::Engine;
    use sha2::{Digest, Sha256};
    let verifier = "test_verifier_string_that_is_long_enough_for_pkce";
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize());

    let params = AuthCodeParams::builder(&code, &client_id, &user_id, redirect_uri, "openid")
        .with_pkce(&challenge, "S256")
        .build();
    repo.store_authorization_code(params)
        .await
        .expect("Failed to store PKCE code");

    let invalid_verifier_result = repo
        .validate_authorization_code(&code, &client_id, Some(redirect_uri), Some("wrong_verifier"))
        .await;

    assert!(
        invalid_verifier_result.is_err(),
        "Invalid verifier should fail"
    );

    cleanup_test_client(&db, &client_id).await;
    cleanup_test_user(&db, &user_id).await;
}

#[tokio::test]
async fn test_refresh_token_lifecycle() {
    let db = setup_test_db().await;
    let client_id = test_client_id();
    create_test_client(&db, &client_id).await;
    let user_id = create_test_user(&db).await;

    let repo = OAuthRepository::new(db.clone()).expect("Failed to create repository");

    let token_id = test_token_id();
    let scopes = "openid profile";
    let expires_at = Utc::now().timestamp() + 7 * 24 * 60 * 60;

    let params =
        RefreshTokenParams::builder(&token_id, &client_id, &user_id, scopes, expires_at).build();
    repo.store_refresh_token(params)
        .await
        .expect("Failed to store refresh token");

    let (returned_user_id, returned_scope) = repo
        .validate_refresh_token(&token_id, &client_id)
        .await
        .expect("Failed to validate refresh token");

    assert_eq!(returned_user_id.as_str(), user_id.as_str());
    assert_eq!(returned_scope, scopes);

    let (user_from_consume, scope_from_consume) = repo
        .consume_refresh_token(&token_id, &client_id)
        .await
        .expect("Failed to consume refresh token");

    assert_eq!(user_from_consume.as_str(), user_id.as_str());
    assert_eq!(scope_from_consume, scopes);

    let validation_after_consume = repo.validate_refresh_token(&token_id, &client_id).await;

    assert!(
        validation_after_consume.is_err(),
        "Consumed token should not validate"
    );

    cleanup_test_client(&db, &client_id).await;
    cleanup_test_user(&db, &user_id).await;
}

#[tokio::test]
async fn test_refresh_token_expiration() {
    let db = setup_test_db().await;
    let client_id = test_client_id();
    create_test_client(&db, &client_id).await;
    let user_id = create_test_user(&db).await;

    let repo = OAuthRepository::new(db.clone()).expect("Failed to create repository");

    let token_id = test_token_id();
    let scopes = "openid";
    let expires_at = Utc::now().timestamp() - 1;

    let params =
        RefreshTokenParams::builder(&token_id, &client_id, &user_id, scopes, expires_at).build();
    repo.store_refresh_token(params)
        .await
        .expect("Failed to store expired token");

    let validation = repo.validate_refresh_token(&token_id, &client_id).await;

    assert!(validation.is_err(), "Expired token should not validate");

    cleanup_test_client(&db, &client_id).await;
    cleanup_test_user(&db, &user_id).await;
}

#[tokio::test]
async fn test_refresh_token_revocation() {
    let db = setup_test_db().await;
    let client_id = test_client_id();
    create_test_client(&db, &client_id).await;
    let user_id = create_test_user(&db).await;

    let repo = OAuthRepository::new(db.clone()).expect("Failed to create repository");

    let token_id = test_token_id();
    let scopes = "openid";
    let expires_at = Utc::now().timestamp() + 7 * 24 * 60 * 60;

    let params =
        RefreshTokenParams::builder(&token_id, &client_id, &user_id, scopes, expires_at).build();
    repo.store_refresh_token(params)
        .await
        .expect("Failed to store token");

    let revoked = repo
        .revoke_refresh_token(&token_id)
        .await
        .expect("Failed to revoke token");

    assert!(revoked, "Token should have been revoked");

    let validation = repo.validate_refresh_token(&token_id, &client_id).await;

    assert!(validation.is_err(), "Revoked token should not validate");

    cleanup_test_client(&db, &client_id).await;
    cleanup_test_user(&db, &user_id).await;
}
