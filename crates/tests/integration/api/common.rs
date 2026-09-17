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
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_app_context, fixture_db_pool};

pub async fn setup_ctx() -> Result<(DbPool, Arc<AppContext>)> {
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let ctx = fixture_app_context(&pool, &b.database_url)?;
    Ok((pool, ctx))
}

pub fn request_context(user: &str) -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("test-agent").expect("valid AgentName"),
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

// Why: the proxy tap stamps `_meta.io.systemprompt/execution.mcp_execution_id`
// into a matched tools/call result before forwarding it, so a forwarded body
// equals the upstream body everywhere but under that key.
pub fn assert_forwarded_with_execution_stamp(forwarded: &[u8], upstream_body: &str) {
    let mut forwarded: serde_json::Value =
        serde_json::from_slice(forwarded).expect("forwarded body is JSON");
    let upstream: serde_json::Value =
        serde_json::from_str(upstream_body).expect("upstream body is JSON");
    let stamped = forwarded["result"]["_meta"]["io.systemprompt/execution"]["mcp_execution_id"]
        .as_str()
        .map(str::to_owned);
    assert!(
        stamped.is_some_and(|id| !id.is_empty()),
        "forwarded result carries the execution stamp: {forwarded}"
    );
    forwarded["result"]
        .as_object_mut()
        .expect("result object")
        .remove("_meta");
    assert_eq!(forwarded, upstream);
}
