//! Race-condition coverage for OAuth authorization codes and refresh tokens.
//!
//! Each test exercises a documented threat from RFC 6749 §10 / RFC 6819
//! (authorization-code reuse, refresh-token rotation, PKCE binding) by
//! issuing concurrent requests against the same row and asserting:
//!
//! 1. exactly one caller succeeds,
//! 2. all losing callers receive a deterministic error (no SQL leakage),
//! 3. the side-effects (refresh-token family revocation, code consumption) are
//!    applied exactly once.

use crate::{cleanup_test_user, create_test_user, setup_test_db};
use base64::Engine;
use chrono::Utc;
use futures::future::join_all;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use systemprompt_identifiers::{AuthorizationCode, ClientId, RefreshTokenId, UserId};
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

fn pkce_pair(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize())
}

async fn create_test_client_with_owner(
    db: &systemprompt_database::DbPool,
    client_id: &ClientId,
    owner: &UserId,
) {
    let repo = ClientRepository::new(db).expect("client repo");
    let params = CreateClientParams {
        client_id: client_id.clone(),
        owner_user_id: owner.clone(),
        client_secret_hash: "test_hash".to_string(),
        client_name: "Test Client".to_string(),
        redirect_uris: vec!["http://localhost:3000/callback".to_string()],
        grant_types: None,
        response_types: None,
        scopes: vec!["openid".to_string()],
        token_endpoint_auth_method: None,
        application_type: "web".to_owned(),
        client_uri: None,
        logo_uri: None,
        contacts: None,
    };
    repo.create(params).await.expect("create client");
}

async fn cleanup_client(db: &systemprompt_database::DbPool, client_id: &ClientId) {
    let repo = ClientRepository::new(db).expect("client repo");
    let _ = repo.delete(client_id).await;
}

#[tokio::test]
async fn test_concurrent_auth_code_exchange_admits_exactly_one() {
    let db = setup_test_db().await;
    let client_id = test_client_id();
    let user_id = create_test_user(&db).await;
    create_test_client_with_owner(
        &db,
        &client_id,
        &systemprompt_test_fixtures::fixture_user_id(),
    )
    .await;

    let repo = Arc::new(OAuthRepository::new(&db).expect("repo"));
    let code = test_code();
    let redirect = "http://localhost:3000/callback";
    repo.store_authorization_code(
        AuthCodeParams::builder(&code, &client_id, &user_id, redirect, "openid").build(),
    )
    .await
    .expect("store code");

    let n = 16;
    let mut handles = Vec::with_capacity(n);
    for _ in 0..n {
        let repo = Arc::clone(&repo);
        let code = code.clone();
        let client_id = client_id.clone();
        handles.push(tokio::spawn(async move {
            repo.validate_authorization_code(&code, &client_id, Some(redirect), None)
                .await
        }));
    }
    let results = join_all(handles).await;

    let (ok, errs): (Vec<_>, Vec<_>) = results
        .into_iter()
        .map(|h| h.expect("join"))
        .partition(Result::is_ok);

    assert_eq!(ok.len(), 1, "exactly one concurrent exchange must succeed");
    assert_eq!(errs.len(), n - 1, "remaining {} must fail", n - 1);
    for e in &errs {
        let msg = e.as_ref().unwrap_err().to_string();
        assert!(
            msg.contains("Invalid authorization code"),
            "loser must surface invalid-grant, got: {msg}"
        );
    }

    cleanup_client(&db, &client_id).await;
    cleanup_test_user(&db, &user_id).await;
}

#[tokio::test]
async fn test_auth_code_expiry_rejected_after_ttl() {
    let db = setup_test_db().await;
    let client_id = test_client_id();
    let user_id = create_test_user(&db).await;
    create_test_client_with_owner(
        &db,
        &client_id,
        &systemprompt_test_fixtures::fixture_user_id(),
    )
    .await;

    let repo = OAuthRepository::new(&db).expect("repo");
    let code = test_code();
    let redirect = "http://localhost:3000/callback";
    repo.store_authorization_code(
        AuthCodeParams::builder(&code, &client_id, &user_id, redirect, "openid").build(),
    )
    .await
    .expect("store code");

    let pool = db.pool_arc().expect("pool");
    let now = Utc::now();
    let rows = sqlx::query!(
        "UPDATE oauth_auth_codes
         SET created_at = $1, expires_at = $2
         WHERE client_id = $3
         RETURNING code",
        now - chrono::Duration::hours(1),
        now - chrono::Duration::seconds(60),
        client_id.as_str(),
    )
    .fetch_all(pool.as_ref())
    .await
    .expect("force-expire code");
    assert_eq!(rows.len(), 1, "exactly one code expected");

    let result = repo
        .validate_authorization_code(&code, &client_id, Some(redirect), None)
        .await;
    let err = result.expect_err("expired code must be rejected");
    assert!(err.to_string().contains("Invalid authorization code"));

    cleanup_client(&db, &client_id).await;
    cleanup_test_user(&db, &user_id).await;
}

#[tokio::test]
async fn test_refresh_token_replay_revokes_family() {
    let db = setup_test_db().await;
    let client_id = test_client_id();
    let user_id = create_test_user(&db).await;
    create_test_client_with_owner(
        &db,
        &client_id,
        &systemprompt_test_fixtures::fixture_user_id(),
    )
    .await;

    let repo = OAuthRepository::new(&db).expect("repo");
    let expires_at = Utc::now().timestamp() + 7 * 24 * 60 * 60;

    let r1 = test_token_id();
    repo.store_refresh_token(
        RefreshTokenParams::builder(&r1, &client_id, &user_id, "openid", expires_at).build(),
    )
    .await
    .expect("store r1");

    let consumed = repo
        .consume_refresh_token(&r1, &client_id)
        .await
        .expect("consume r1");
    let family = consumed.family_id.clone();

    let r2 = test_token_id();
    repo.store_refresh_token(
        RefreshTokenParams::builder(&r2, &client_id, &user_id, "openid", expires_at)
            .with_family(&family)
            .build(),
    )
    .await
    .expect("store r2");

    let validate_r2_before = repo.validate_refresh_token(&r2, &client_id).await;
    assert!(
        validate_r2_before.is_ok(),
        "r2 must be live before replay: {validate_r2_before:?}"
    );

    let replay = repo.consume_refresh_token(&r1, &client_id).await;
    assert!(replay.is_err(), "replay of consumed r1 must fail");
    assert!(
        replay
            .unwrap_err()
            .to_string()
            .contains("Invalid refresh token"),
        "replay must surface invalid-grant",
    );

    let validate_r2_after = repo.validate_refresh_token(&r2, &client_id).await;
    assert!(
        validate_r2_after.is_err(),
        "r2 must be revoked after r1 replay (family kill)",
    );

    cleanup_client(&db, &client_id).await;
    cleanup_test_user(&db, &user_id).await;
}

#[tokio::test]
async fn test_concurrent_refresh_rotation_admits_exactly_one() {
    let db = setup_test_db().await;
    let client_id = test_client_id();
    let user_id = create_test_user(&db).await;
    create_test_client_with_owner(
        &db,
        &client_id,
        &systemprompt_test_fixtures::fixture_user_id(),
    )
    .await;

    let repo = Arc::new(OAuthRepository::new(&db).expect("repo"));
    let expires_at = Utc::now().timestamp() + 7 * 24 * 60 * 60;

    let r1 = test_token_id();
    repo.store_refresh_token(
        RefreshTokenParams::builder(&r1, &client_id, &user_id, "openid", expires_at).build(),
    )
    .await
    .expect("store r1");

    let n = 8;
    let mut handles = Vec::with_capacity(n);
    for _ in 0..n {
        let repo = Arc::clone(&repo);
        let r1 = r1.clone();
        let client_id = client_id.clone();
        handles.push(tokio::spawn(async move {
            repo.consume_refresh_token(&r1, &client_id).await
        }));
    }
    let results = join_all(handles).await;
    let (ok, errs): (Vec<_>, Vec<_>) = results
        .into_iter()
        .map(|h| h.expect("join"))
        .partition(Result::is_ok);

    assert_eq!(
        ok.len(),
        1,
        "concurrent refresh rotation must elect exactly one winner"
    );
    assert_eq!(errs.len(), n - 1);
    for e in &errs {
        let msg = e.as_ref().unwrap_err().to_string();
        assert!(
            msg.contains("Invalid refresh token"),
            "loser must return invalid-grant; got: {msg}"
        );
    }

    let post_concurrency_validate = repo.validate_refresh_token(&r1, &client_id).await;
    assert!(
        post_concurrency_validate.is_err(),
        "winning consumption + loser-triggered family revoke must invalidate r1"
    );

    cleanup_client(&db, &client_id).await;
    cleanup_test_user(&db, &user_id).await;
}

#[tokio::test]
async fn test_concurrent_pkce_verifier_mismatch_never_admits_wrong_verifier() {
    let db = setup_test_db().await;
    let client_id = test_client_id();
    let user_id = create_test_user(&db).await;
    create_test_client_with_owner(
        &db,
        &client_id,
        &systemprompt_test_fixtures::fixture_user_id(),
    )
    .await;

    let repo = Arc::new(OAuthRepository::new(&db).expect("repo"));
    let code = test_code();
    let redirect = "http://localhost:3000/callback";
    let verifier = "this_is_the_correct_pkce_verifier_string_value";
    let challenge = pkce_pair(verifier);
    repo.store_authorization_code(
        AuthCodeParams::builder(&code, &client_id, &user_id, redirect, "openid")
            .with_pkce(&challenge, "S256")
            .build(),
    )
    .await
    .expect("store pkce code");

    let repo_correct = Arc::clone(&repo);
    let repo_wrong = Arc::clone(&repo);
    let code_a = code.clone();
    let code_b = code.clone();
    let client_a = client_id.clone();
    let client_b = client_id.clone();
    let h_correct = tokio::spawn(async move {
        repo_correct
            .validate_authorization_code(&code_a, &client_a, Some(redirect), Some(verifier))
            .await
    });
    let h_wrong = tokio::spawn(async move {
        repo_wrong
            .validate_authorization_code(
                &code_b,
                &client_b,
                Some(redirect),
                Some("totally_wrong_verifier_xxxxxxxxxxxxxxxxxxxxxxxxxxx"),
            )
            .await
    });

    let r_correct = h_correct.await.expect("join");
    let r_wrong = h_wrong.await.expect("join");

    let correct_ok = r_correct.is_ok();
    let wrong_ok = r_wrong.is_ok();

    assert!(
        !wrong_ok,
        "wrong-verifier exchange must never succeed (got Ok)"
    );
    if !correct_ok {
        assert!(
            r_correct
                .as_ref()
                .err()
                .unwrap()
                .to_string()
                .contains("Invalid authorization code"),
        );
    }

    cleanup_client(&db, &client_id).await;
    cleanup_test_user(&db, &user_id).await;
}

#[tokio::test]
async fn test_dynamic_client_registration_owner_not_hijackable() {
    let db = setup_test_db().await;
    let client_id = test_client_id();
    let owner_a = create_test_user(&db).await;
    let owner_b = create_test_user(&db).await;

    create_test_client_with_owner(&db, &client_id, &owner_a).await;

    let repo = ClientRepository::new(&db).expect("client repo");
    let hijack = repo
        .create(CreateClientParams {
            client_id: client_id.clone(),
            owner_user_id: owner_b.clone(),
            client_secret_hash: "alt_hash".to_string(),
            client_name: "Hijack Attempt".to_string(),
            redirect_uris: vec!["http://attacker.example/cb".to_string()],
            grant_types: None,
            response_types: None,
            scopes: vec!["openid".to_string()],
            token_endpoint_auth_method: None,
            application_type: "web".to_owned(),
            client_uri: None,
            logo_uri: None,
            contacts: None,
        })
        .await;
    assert!(
        hijack.is_err(),
        "second registration of an existing client_id must fail"
    );

    let row = repo
        .get_by_client_id(&client_id)
        .await
        .expect("read client")
        .expect("client present");
    assert_eq!(
        row.owner_user_id.as_str(),
        owner_a.as_str(),
        "owner_user_id must remain the original creator"
    );

    cleanup_client(&db, &client_id).await;
    cleanup_test_user(&db, &owner_a).await;
    cleanup_test_user(&db, &owner_b).await;
}

#[tokio::test]
async fn test_concurrent_client_registration_race() {
    let db = setup_test_db().await;
    let client_id = test_client_id();
    let owners = vec![
        create_test_user(&db).await,
        create_test_user(&db).await,
        create_test_user(&db).await,
        create_test_user(&db).await,
    ];

    let repo = Arc::new(ClientRepository::new(&db).expect("client repo"));
    let mut handles = Vec::with_capacity(owners.len());
    for owner in &owners {
        let repo = Arc::clone(&repo);
        let client_id = client_id.clone();
        let owner = owner.clone();
        handles.push(tokio::spawn(async move {
            repo.create(CreateClientParams {
                client_id,
                owner_user_id: owner,
                client_secret_hash: "h".to_string(),
                client_name: "race".to_string(),
                redirect_uris: vec!["http://localhost/cb".to_string()],
                grant_types: None,
                response_types: None,
                scopes: vec!["openid".to_string()],
                token_endpoint_auth_method: None,
                application_type: "web".to_owned(),
                client_uri: None,
                logo_uri: None,
                contacts: None,
            })
            .await
        }));
    }

    let results = join_all(handles).await;
    let oks = results
        .iter()
        .filter(|r| r.as_ref().unwrap().is_ok())
        .count();
    assert_eq!(oks, 1, "exactly one concurrent registration may succeed");

    let read_repo = ClientRepository::new(&db).expect("client repo");
    let row = read_repo
        .get_by_client_id(&client_id)
        .await
        .expect("read")
        .expect("present");
    assert!(
        owners
            .iter()
            .any(|o| o.as_str() == row.owner_user_id.as_str()),
        "owner must be one of the racing users",
    );

    cleanup_client(&db, &client_id).await;
    for o in &owners {
        cleanup_test_user(&db, o).await;
    }
}
