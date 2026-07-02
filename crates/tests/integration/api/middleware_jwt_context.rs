//! `JwtContextExtractor` header, gateway, and A2A-request extraction paths.
//!
//! Builds the extractor from a fixture `AppContext` and exercises: a missing
//! bearer, an undecodable token, a fully valid seeded credential (standard and
//! gateway decode), the `x-context-id`-without-auth forbidden-header guard, and
//! the A2A body context-id sources (direct `contextId` and `tasks/*`).

use std::sync::Arc;

use anyhow::Result;
use axum::body::Body;
use axum::extract::Request;
use axum::http::HeaderMap;
use systemprompt_api::services::middleware::{
    ContextExtractor, JtiRevocationChecker, JwtContextExtractor,
};
use systemprompt_identifiers::JwtToken;
use systemprompt_test_fixtures::{install_test_signing_key, seed_admin_credential};
use systemprompt_traits::{AnalyticsProvider, UserProvider};
use systemprompt_users::UserService;

use super::common::setup_ctx;

async fn extractor() -> Result<(systemprompt_database::DbPool, JwtContextExtractor)> {
    let (db, ctx) = setup_ctx().await?;
    install_test_signing_key();
    let concrete = Arc::clone(ctx.analytics_service());
    let analytics: Arc<dyn AnalyticsProvider> = concrete;
    let user_provider: Arc<dyn UserProvider> = Arc::new(UserService::new(ctx.db_pool())?);
    let jti = JtiRevocationChecker::from_pool(ctx.db_pool())?;
    Ok((db, JwtContextExtractor::new(analytics, user_provider, jti)))
}

fn bearer(headers: &mut HeaderMap, token: &str) {
    headers.insert(
        "authorization",
        format!("Bearer {token}").parse().expect("header"),
    );
}

#[tokio::test]
async fn missing_bearer_is_rejected() -> Result<()> {
    let (_db, extractor) = extractor().await?;
    let headers = HeaderMap::new();
    let result = extractor.extract_standard(&headers).await;
    assert!(result.is_err(), "no auth header must fail");
    Ok(())
}

#[tokio::test]
async fn garbage_token_is_invalid() -> Result<()> {
    let (_db, extractor) = extractor().await?;
    let mut headers = HeaderMap::new();
    bearer(&mut headers, "not.a.jwt");
    let result = extractor.extract_standard(&headers).await;
    assert!(result.is_err(), "undecodable token must fail");
    Ok(())
}

#[tokio::test]
async fn valid_credential_extracts_standard_context() -> Result<()> {
    let (db, extractor) = extractor().await?;
    let fixture = seed_admin_credential(&db, "jwt-standard").await?;
    let mut headers = HeaderMap::new();
    bearer(&mut headers, fixture.jwt.as_str());
    headers.insert(
        "x-context-id",
        "11111111-1111-1111-1111-111111111111".parse().expect("h"),
    );
    let ctx = extractor.extract_standard(&headers).await?;
    assert_eq!(ctx.user_id().as_str(), fixture.user_id.as_str());
    assert_eq!(
        ctx.context_id().as_str(),
        "11111111-1111-1111-1111-111111111111"
    );
    Ok(())
}

#[tokio::test]
async fn valid_credential_decodes_for_gateway() -> Result<()> {
    let (db, extractor) = extractor().await?;
    let fixture = seed_admin_credential(&db, "jwt-gateway").await?;
    let token = JwtToken::new(fixture.jwt.as_str().to_owned());
    let (jwt_ctx, user) = extractor.decode_for_gateway(&token).await?;
    assert_eq!(jwt_ctx.user_id.as_str(), fixture.user_id.as_str());
    assert_eq!(user.id.as_str(), fixture.user_id.as_str());
    Ok(())
}

#[tokio::test]
async fn context_id_header_without_auth_is_forbidden() -> Result<()> {
    let (_db, extractor) = extractor().await?;
    let request = Request::builder()
        .uri("/a2a")
        .header("x-context-id", "abc")
        .body(Body::empty())?;
    let result = extractor.extract_from_request(request).await;
    assert!(result.is_err(), "x-context-id without auth is forbidden");
    Ok(())
}

#[tokio::test]
async fn a2a_request_reads_direct_context_id_from_body() -> Result<()> {
    let (db, extractor) = extractor().await?;
    let fixture = seed_admin_credential(&db, "jwt-a2a-direct").await?;
    let body = serde_json::json!({
        "method": "message/send",
        "params": { "message": { "contextId": "22222222-2222-2222-2222-222222222222" } }
    })
    .to_string();
    let request = Request::builder()
        .uri("/a2a")
        .header("authorization", format!("Bearer {}", fixture.jwt.as_str()))
        .body(Body::from(body))?;
    let (ctx, _req) = extractor.extract_from_request(request).await?;
    assert_eq!(
        ctx.context_id().as_str(),
        "22222222-2222-2222-2222-222222222222"
    );
    Ok(())
}

#[tokio::test]
async fn a2a_request_reads_task_id_from_task_method() -> Result<()> {
    let (db, extractor) = extractor().await?;
    let fixture = seed_admin_credential(&db, "jwt-a2a-task").await?;
    let body = serde_json::json!({
        "method": "tasks/get",
        "params": { "id": "task-abc-123" }
    })
    .to_string();
    let request = Request::builder()
        .uri("/a2a")
        .header("authorization", format!("Bearer {}", fixture.jwt.as_str()))
        .body(Body::from(body))?;
    let (ctx, _req) = extractor.extract_from_request(request).await?;
    assert_eq!(ctx.task_id().map(|t| t.as_str()), Some("task-abc-123"));
    Ok(())
}

#[tokio::test]
async fn a2a_request_missing_context_id_is_rejected() -> Result<()> {
    let (db, extractor) = extractor().await?;
    let fixture = seed_admin_credential(&db, "jwt-a2a-missing").await?;
    let body = serde_json::json!({ "method": "message/send", "params": {} }).to_string();
    let request = Request::builder()
        .uri("/a2a")
        .header("authorization", format!("Bearer {}", fixture.jwt.as_str()))
        .body(Body::from(body))?;
    let result = extractor.extract_from_request(request).await;
    assert!(result.is_err(), "no contextId in body must fail");
    Ok(())
}
