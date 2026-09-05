//! Tests for the gateway session-minting endpoint.
//!
//! `create_session` trades an API key for a session id. It deliberately refuses
//! JWT callers — they already carry a session — so the prefix check is the line
//! between "mint a session" and "you already have one", and getting it wrong
//! would mint duplicate sessions for every bearer-token request.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use axum::extract::Request;
use axum::http::{StatusCode, header};
use std::sync::Arc;
use systemprompt_api::routes::gateway::sessions::create_session;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context, fixture_database_url, fixture_db_pool,
    seed_user_row,
};
use systemprompt_users::{API_KEY_PREFIX, ApiKeyService, IssueApiKeyParams, UserRepository};
use uuid::Uuid;

struct Harness {
    ctx: systemprompt_runtime::AppContext,
    pool: DbPool,
    user_id: UserId,
}

async fn harness_or_skip() -> Option<Harness> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let user_id = UserId::new(Uuid::new_v4().to_string());
    let email = format!("mint-{}@sessions.invalid", Uuid::new_v4().simple());
    seed_user_row(&pool, &user_id, &email).await.expect("user");
    let ctx = fixture_app_context(&pool, &url).expect("app context");
    Some(Harness {
        ctx: (*ctx).clone(),
        pool,
        user_id,
    })
}

fn request_with(headers: &[(header::HeaderName, &str)]) -> Request {
    let mut builder = Request::builder().uri("/sessions").method("POST");
    for (name, value) in headers {
        builder = builder.header(name, *value);
    }
    builder.body(axum::body::Body::empty()).unwrap()
}

async fn mint_key(pool: &DbPool, user_id: &UserId) -> String {
    ApiKeyService::new(Arc::new(
        UserRepository::new(pool).expect("user repository"),
    ))
    .issue(IssueApiKeyParams {
        user_id,
        name: "gateway-mint",
        expires_at: None,
    })
    .await
    .expect("issue")
    .secret
}

#[tokio::test]
async fn valid_api_key_mints_a_session() {
    let Some(h) = harness_or_skip().await else {
        return;
    };
    let secret = mint_key(&h.pool, &h.user_id).await;

    let (status, body) = create_session(
        h.ctx,
        request_with(&[(header::HeaderName::from_static("x-api-key"), &secret)]),
    )
    .await
    .expect("a valid key should mint a session");

    assert_eq!(status, StatusCode::CREATED);
    assert!(
        !body.0.session_id.as_str().is_empty(),
        "a minted session must carry an id"
    );
}

#[tokio::test]
async fn api_key_via_bearer_header_also_mints() {
    let Some(h) = harness_or_skip().await else {
        return;
    };
    let secret = mint_key(&h.pool, &h.user_id).await;

    let (status, _body) = create_session(
        h.ctx,
        request_with(&[(header::AUTHORIZATION, &format!("Bearer {secret}"))]),
    )
    .await
    .expect("an API key presented as a bearer token should still mint");

    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn missing_credential_is_unauthorized() {
    let Some(h) = harness_or_skip().await else {
        return;
    };
    let err = create_session(h.ctx, request_with(&[]))
        .await
        .expect_err("no credential must be refused");
    assert!(
        format!("{err:?}").contains("x-api-key") || format!("{err:?}").contains("Bearer"),
        "the error should name the accepted credential forms: {err:?}"
    );
}

#[tokio::test]
async fn jwt_shaped_credential_is_refused_rather_than_minting() {
    let Some(h) = harness_or_skip().await else {
        return;
    };
    // A JWT caller already holds a session; minting another would double-count
    // the session and split that caller's analytics across two rows.
    let err = create_session(
        h.ctx,
        request_with(&[(
            header::AUTHORIZATION,
            "Bearer eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.e30.sig",
        )]),
    )
    .await
    .expect_err("a JWT must not mint a second session");

    assert!(
        format!("{err:?}").contains("API keys only"),
        "the refusal should explain that JWT callers already carry a session: {err:?}"
    );
}

#[tokio::test]
async fn unknown_api_key_is_unauthorized() {
    let Some(h) = harness_or_skip().await else {
        return;
    };
    let bogus = format!("{API_KEY_PREFIX}{}", Uuid::new_v4().simple());

    let err = create_session(
        h.ctx,
        request_with(&[(header::HeaderName::from_static("x-api-key"), &bogus)]),
    )
    .await
    .expect_err("an unissued key must be refused");

    assert!(
        format!("{err:?}").contains("Invalid or revoked"),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn revoked_api_key_no_longer_mints() {
    let Some(h) = harness_or_skip().await else {
        return;
    };
    let service = ApiKeyService::new(Arc::new(
        UserRepository::new(&h.pool).expect("user repository"),
    ));
    let minted = service
        .issue(IssueApiKeyParams {
            user_id: &h.user_id,
            name: "to-revoke",
            expires_at: None,
        })
        .await
        .expect("issue");
    service
        .revoke(&minted.record.id, &h.user_id)
        .await
        .expect("revoke");

    let err = create_session(
        h.ctx,
        request_with(&[(header::HeaderName::from_static("x-api-key"), &minted.secret)]),
    )
    .await
    .expect_err("a revoked key must stop minting sessions");

    assert!(
        format!("{err:?}").contains("Invalid or revoked"),
        "unexpected error: {err:?}"
    );
}
