//! Direct-call coverage for the bridge credential-exchange handlers in
//! `routes::gateway::auth`.
//!
//! Each handler takes the `AppContext` by value, so the tests invoke them
//! directly (bypassing the router) and assert exact status codes. The fixture
//! context carries a live analytics provider, so `require_analytics` succeeds
//! and the PAT happy path reaches `issue_bridge_access`.

use anyhow::Result;
use axum::Json;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use std::sync::Arc;
use systemprompt_api::routes::gateway::{auth, bridge_manifest};
use systemprompt_api::services::middleware::{JtiRevocationChecker, JwtContextExtractor};
use systemprompt_traits::AppContext as _;
use systemprompt_users::{ApiKeyService, IssueApiKeyParams};

use super::common::setup_ctx;

fn no_auth_request() -> Request {
    Request::builder()
        .uri("/pat")
        .body(axum::body::Body::empty())
        .expect("request build")
}

fn bearer_request(token: &str) -> Request {
    Request::builder()
        .uri("/pat")
        .header("authorization", format!("Bearer {token}"))
        .body(axum::body::Body::empty())
        .expect("request build")
}

#[tokio::test]
async fn capabilities_lists_supported_modes() -> Result<()> {
    let Json(caps) = auth::capabilities().await;
    assert!(caps.modes.contains(&"pat"));
    assert!(caps.modes.contains(&"session"));
    Ok(())
}

#[tokio::test]
async fn pat_without_bearer_is_unauthorized() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let err = auth::pat((*ctx).clone(), no_auth_request())
        .await
        .expect_err("missing bearer must error");
    assert_eq!(err.into_response().status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn pat_with_invalid_token_is_unauthorized() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let err = auth::pat((*ctx).clone(), bearer_request("sk_not_a_real_key"))
        .await
        .expect_err("invalid pat must error");
    assert_eq!(err.into_response().status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn pat_with_valid_key_issues_bridge_access() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    systemprompt_test_fixtures::install_test_signing_key();
    let uniq = uuid::Uuid::new_v4();
    let user = systemprompt_identifiers::UserId::new(uniq.to_string());
    systemprompt_test_fixtures::seed_user_row(&pool, &user, &format!("pat-{uniq}@example.invalid"))
        .await?;

    let service = ApiKeyService::new(ctx.db_pool())?;
    let issued = service
        .issue(IssueApiKeyParams {
            user_id: &user,
            name: "bridge pat",
            expires_at: None,
        })
        .await?;

    let resp = auth::pat((*ctx).clone(), bearer_request(&issued.secret))
        .await
        .expect("valid pat must issue bridge access");
    assert_eq!(resp.into_response().status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn session_with_blank_code_is_bad_request() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let err = auth::session(
        (*ctx).clone(),
        HeaderMap::new(),
        Json(auth::SessionExchangeBody {
            code: "   ".to_owned(),
        }),
    )
    .await
    .expect_err("blank code must error");
    assert_eq!(err.into_response().status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn session_with_unknown_code_is_unauthorized() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let err = auth::session(
        (*ctx).clone(),
        HeaderMap::new(),
        Json(auth::SessionExchangeBody {
            code: format!("code-{}", uuid::Uuid::new_v4()),
        }),
    )
    .await
    .expect_err("unknown code must error");
    assert_eq!(err.into_response().status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn session_pat_with_blank_code_is_bad_request() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let err = auth::session_pat(
        (*ctx).clone(),
        Json(auth::SessionPatBody {
            code: String::new(),
            device_name: None,
        }),
    )
    .await
    .expect_err("blank code must error");
    assert_eq!(err.into_response().status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn session_pat_with_unknown_code_is_unauthorized() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let err = auth::session_pat(
        (*ctx).clone(),
        Json(auth::SessionPatBody {
            code: format!("code-{}", uuid::Uuid::new_v4()),
            device_name: Some("my device".to_owned()),
        }),
    )
    .await
    .expect_err("unknown code must error");
    assert_eq!(err.into_response().status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn mtls_with_blank_fingerprint_is_bad_request() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let err = auth::mtls(
        (*ctx).clone(),
        HeaderMap::new(),
        Json(auth::MtlsRequestBody {
            device_cert_fingerprint: "  ".to_owned(),
        }),
    )
    .await
    .expect_err("blank fingerprint must error");
    assert_eq!(err.into_response().status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn mtls_with_unenrolled_cert_is_unauthorized() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let err = auth::mtls(
        (*ctx).clone(),
        HeaderMap::new(),
        Json(auth::MtlsRequestBody {
            device_cert_fingerprint: "a".repeat(64),
        }),
    )
    .await
    .expect_err("unenrolled cert must error");
    assert_eq!(err.into_response().status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn provision_oauth_client_without_bearer_is_unauthorized() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let extractor = Arc::new(JwtContextExtractor::new(
        ctx.analytics_provider().expect("analytics provider"),
        ctx.user_provider().expect("user provider"),
        JtiRevocationChecker::from_pool(ctx.db_pool())?,
    ));
    let err = auth::provision_oauth_client(extractor, (*ctx).clone(), no_auth_request())
        .await
        .expect_err("missing bearer must error");
    assert_eq!(err.into_response().status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

fn jwt_extractor(
    ctx: &systemprompt_runtime::AppContext,
) -> anyhow::Result<Arc<JwtContextExtractor>> {
    Ok(Arc::new(JwtContextExtractor::new(
        ctx.analytics_provider().expect("analytics provider"),
        ctx.user_provider().expect("user provider"),
        JtiRevocationChecker::from_pool(ctx.db_pool())?,
    )))
}

async fn seed_exchange_code(
    ctx: &systemprompt_runtime::AppContext,
    pool: &systemprompt_database::DbPool,
) -> Result<(String, systemprompt_identifiers::UserId)> {
    let uniq = uuid::Uuid::new_v4();
    let user = systemprompt_identifiers::UserId::new(uniq.to_string());
    systemprompt_test_fixtures::seed_user_row(pool, &user, &format!("ex-{uniq}@example.invalid"))
        .await?;
    let code = format!("code-{uniq}");
    let repo = systemprompt_oauth::OAuthRepository::new(ctx.db_pool())?;
    repo.create_bridge_exchange_code(systemprompt_oauth::repository::CreateExchangeCodeParams {
        code_hash: &systemprompt_oauth::services::hash_exchange_code(&code),
        user_id: &user,
        expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
    })
    .await?;
    Ok((code, user))
}

#[tokio::test]
async fn session_pat_with_valid_code_mints_pat() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (code, _user) = seed_exchange_code(&ctx, &pool).await?;
    let resp = auth::session_pat(
        (*ctx).clone(),
        Json(auth::SessionPatBody {
            code,
            device_name: Some("cov device".to_owned()),
        }),
    )
    .await
    .expect("valid exchange code must mint a pat");
    assert_eq!(resp.into_response().status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn session_with_valid_code_issues_bridge_access() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    systemprompt_test_fixtures::install_test_signing_key();
    let (code, _user) = seed_exchange_code(&ctx, &pool).await?;
    let resp = auth::session(
        (*ctx).clone(),
        HeaderMap::new(),
        Json(auth::SessionExchangeBody { code }),
    )
    .await
    .expect("valid exchange code must issue bridge access");
    assert_eq!(resp.into_response().status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn manifest_without_credential_is_unauthorized() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let extractor = jwt_extractor(&ctx)?;
    let (status, _msg) = bridge_manifest::manifest(extractor, (*ctx).clone(), HeaderMap::new())
        .await
        .expect_err("missing credential must error");
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}
