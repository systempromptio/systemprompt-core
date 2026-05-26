//! Shared helpers for API route integration tests.

use std::sync::Arc;

use anyhow::Result;
use axum::body::Body;
use axum::http::{Request, Response};
use http_body_util::BodyExt;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{fixture_app_context, fixture_db_pool, fixture_database_url};

pub async fn setup_ctx() -> Result<(DbPool, Arc<AppContext>)> {
    let url = fixture_database_url()?;
    let pool = fixture_db_pool(&url).await?;
    let ctx = fixture_app_context(&pool, &url)?;
    Ok((pool, ctx))
}

pub fn request_context(user: &str) -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("test-agent"),
    )
    .with_actor(systemprompt_identifiers::Actor::user(UserId::new(user)))
}

pub async fn body_to_string(resp: Response<Body>) -> Result<(http::StatusCode, String)> {
    let status = resp.status();
    let body = resp.into_body().collect().await?.to_bytes();
    Ok((status, String::from_utf8_lossy(&body).into_owned()))
}

pub fn empty_get(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("request build")
}

pub fn empty_delete(uri: &str) -> Request<Body> {
    Request::builder()
        .method(http::Method::DELETE)
        .uri(uri)
        .body(Body::empty())
        .expect("request build")
}

pub fn json_post(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(http::Method::POST)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request build")
}
