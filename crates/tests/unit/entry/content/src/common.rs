//! Shared helpers for the content route and command tests.

use std::sync::Arc;

use anyhow::Result;
use axum::body::Body;
use axum::http::{Request, Response};
use http_body_util::BodyExt;
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides};
use systemprompt_content::repository::ContentRepositories;
use systemprompt_content_entry::http::ContentHttpState;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_config, test_app_context, test_db_pool,
};

pub async fn setup_ctx() -> Result<(DbPool, Arc<AppContext>)> {
    let b = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let ctx = test_app_context(&pool, &b.database_url);
    Ok((pool, ctx))
}

pub fn content_state(ctx: &AppContext) -> ContentHttpState {
    ContentHttpState::new(Arc::clone(ctx.content_repositories()), ctx.config())
}

pub fn pool_state(pool: &DbPool) -> ContentHttpState {
    let config = fixture_config(&ensure_test_bootstrap().database_url);
    ContentHttpState::new(Arc::new(ContentRepositories::new(pool)), &config)
}

pub fn with_app_boot(cli: CliConfig, env: EnvOverrides, app: Arc<AppContext>) -> CommandContext {
    let boot = app.boot().clone();
    CommandContext::with_app_context(cli, env, app).with_boot(Some(boot))
}

pub fn request_context(user: &str) -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("test-agent").expect("valid AgentName"),
        systemprompt_identifiers::Actor::user(UserId::new(user)),
    )
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

pub fn json_post(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(http::Method::POST)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request build")
}
