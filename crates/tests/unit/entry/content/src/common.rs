//! Shared helpers for the content route and command tests.

use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response};
use http_body_util::BodyExt;
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_content::repository::ContentRepositories;
use systemprompt_content_entry::http::ContentHttpState;
use systemprompt_database::DbPool;
use systemprompt_extension::cli::CliConfig;
use systemprompt_extension::router::RouteSet;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_manifest::Config;
use systemprompt_models::RequestContext;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_runtime::{AppContext, Kernel};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_config, test_app_context, test_db_pool,
};
use systemprompt_traits::{BackgroundTasks, DatabaseHandle};

pub async fn setup_ctx() -> Result<(DbPool, Arc<AppContext>)> {
    let b = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let ctx = test_app_context(&b.database_url).await;
    Ok((pool, ctx))
}

pub fn content_state(ctx: &AppContext) -> ContentHttpState {
    ContentHttpState::new(Arc::clone(ctx.content_repositories()), ctx.config())
}

pub fn negotiating_state(ctx: &AppContext) -> ContentHttpState {
    let mut config = ctx.config().clone();
    config.content_negotiation.enabled = true;
    config.content_negotiation.markdown_suffix = ".md".to_owned();
    ContentHttpState::new(Arc::clone(ctx.content_repositories()), &config)
}

pub fn pool_state(pool: &DbPool) -> ContentHttpState {
    let config = fixture_config(&ensure_test_bootstrap().database_url);
    ContentHttpState::new(Arc::new(ContentRepositories::new(pool)), &config)
}

pub fn content_kernel(
    pool: &DbPool,
    repositories: Arc<ContentRepositories>,
    config: &Config,
) -> Kernel {
    let handle: Arc<dyn DatabaseHandle> = Arc::<systemprompt_database::Database>::clone(pool);
    Kernel::new(
        handle,
        BackgroundTasks::new(),
        Dependencies::default()
            .with(repositories)
            .with(Arc::new(config.clone())),
    )
}

pub fn mount(
    set: RouteSet,
    pool: &DbPool,
    repositories: Arc<ContentRepositories>,
    config: &Config,
) -> Router {
    let kernel = content_kernel(pool, repositories, config);
    set.materialize(&kernel.resolved())
        .expect("the content state resolves from the repositories and the config")
        .into_iter()
        .fold(Router::new(), Router::merge)
}

pub fn content_app(set: RouteSet, ctx: &AppContext) -> Router {
    mount(
        set,
        ctx.db_pool(),
        Arc::clone(ctx.content_repositories()),
        ctx.config(),
    )
}

pub fn with_app_boot(cli: CliConfig, env: EnvOverrides, app: Arc<AppContext>) -> CommandContext {
    let boot = app.boot().clone();
    let admitted = Arc::clone(app.admitted());
    let kernel = content_kernel(
        app.db_pool(),
        Arc::clone(app.content_repositories()),
        app.config(),
    );
    CommandContext::with_app_context(cli, env, app)
        .with_boot(Some(boot))
        .with_admitted(Some(admitted))
        .with_kernel(Arc::new(kernel))
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
