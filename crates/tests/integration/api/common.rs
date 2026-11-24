//! Shared helpers for API route integration tests.

use std::sync::Arc;

use anyhow::Result;
use axum::body::Body;
use axum::http::{Request, Response};
use http_body_util::BodyExt;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_manifest::Config;
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_config, fixture_config, test_app_context,
    test_db_pool,
};

pub fn reference_admitted(
    disabled: &[systemprompt_identifiers::ExtensionId],
) -> Result<Arc<systemprompt_extension::Admitted>> {
    let mut profile = (*systemprompt_test_fixtures::test_boot_profile()).clone();
    profile.extensions.disabled = disabled
        .iter()
        .cloned()
        .map(systemprompt_manifest::DisabledExtension::literal)
        .collect();
    Ok(Arc::new(systemprompt::reference().build(&profile)?))
}

pub async fn setup_ctx() -> Result<(DbPool, Arc<AppContext>)> {
    let b = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let ctx = test_app_context(&b.database_url).await;
    Ok((pool, ctx))
}

pub async fn agent_app_context(database_url: &str) -> Arc<AppContext> {
    ensure_test_bootstrap();
    systemprompt_test_fixtures::fixture_app_context_with_extensions(
        fixture_config(database_url),
        reference_admitted(&[]).expect("the reference composition admits"),
    )
    .await
    .expect("app context with the agent-http member admitted")
}

pub async fn setup_agent_ctx() -> Result<(DbPool, Arc<AppContext>)> {
    let b = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let ctx = agent_app_context(&b.database_url).await;
    Ok((pool, ctx))
}

pub async fn setup_drive_ctx() -> Result<(DbPool, Arc<AppContext>)> {
    let b = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut members = systemprompt_test_fixtures::fixture_core_members();
    members.push(Box::new(
        super::documents_drive_policy::DriveSyncPolicyMember,
    ));
    let ctx = systemprompt_test_fixtures::fixture_app_context_with_extensions(
        fixture_config(&b.database_url),
        systemprompt_test_fixtures::fixture_admitted_of(members),
    )
    .await?;
    Ok((pool, ctx))
}

pub async fn setup_ctx_customised(
    customize: impl FnOnce(&mut Config),
) -> Result<(DbPool, Arc<AppContext>)> {
    let b = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&b.database_url);
    customize(&mut config);
    let ctx = fixture_app_context_with_config(config).await?;
    Ok((pool, ctx))
}

pub fn with_config<S: Clone + Send + Sync + 'static>(
    router: axum::Router<S>,
    ctx: &AppContext,
) -> axum::Router<S> {
    router.layer(axum::Extension(Arc::clone(ctx.config_arc())))
}

pub fn browser_page<S: Clone + Send + Sync + 'static>(router: axum::Router<S>) -> axum::Router<S> {
    let config = Arc::new(fixture_config(&ensure_test_bootstrap().database_url));
    router
        .layer(axum::middleware::from_fn(same_origin_fetch))
        .layer(axum::Extension(config))
}

async fn same_origin_fetch(
    mut request: Request<Body>,
    next: axum::middleware::Next,
) -> Response<Body> {
    request
        .headers_mut()
        .entry("sec-fetch-site")
        .or_insert_with(|| axum::http::HeaderValue::from_static("same-origin"));
    next.run(request).await
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

pub async fn read_json(resp: Response<Body>) -> Result<serde_json::Value> {
    let bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await?;
    Ok(serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
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

pub fn mounted_set(
    ctx: &AppContext,
    set: systemprompt_extension::router::RouteSet,
) -> axum::Router {
    set.materialize(&ctx.kernel().resolved())
        .expect("the admitted kernel holds the route set's state")
        .into_iter()
        .fold(axum::Router::new(), axum::Router::merge)
}

pub fn webhook_app(ctx: &AppContext) -> axum::Router {
    systemprompt_api::routes::webhook::router(
        systemprompt_api::routes::webhook::WebhookState::from_context(ctx)
            .expect("the admitted kernel holds the webhook state"),
    )
}

pub fn agent_http_state(ctx: &AppContext) -> systemprompt_agent_entry::http::AgentHttpState {
    <systemprompt_agent_entry::http::AgentHttpState as systemprompt_extension::FromState>::from_state(
        &ctx.kernel().resolved(),
    )
    .expect("the admitted kernel holds the agent route state")
}

pub fn documents_state(
    ctx: &AppContext,
) -> Result<systemprompt_files_entry::http::documents::DocumentsState> {
    let owner = systemprompt_identifiers::ExtensionId::new(
        systemprompt_extension::reserved::CoreId::FilesHttp.id(),
    );
    let state = systemprompt_documents::http::surface::FilesRouteState {
        resolved: ctx
            .kernel()
            .route_scoped(&owner, systemprompt_extension::StandingTag::Core, &[]),
        host: systemprompt_api::routes::documents::files_host_handles(ctx),
    };
    systemprompt_files_entry::http::documents::DocumentsState::build(&state)?
        .ok_or_else(|| anyhow::anyhow!("the document store is disabled"))
}

pub fn issues_state(
    ctx: &AppContext,
) -> Result<systemprompt_files_entry::http::issues::IssuesState> {
    let owner = systemprompt_identifiers::ExtensionId::new(
        systemprompt_extension::reserved::CoreId::FilesHttp.id(),
    );
    let state = systemprompt_documents::http::surface::FilesRouteState {
        resolved: ctx
            .kernel()
            .route_scoped(&owner, systemprompt_extension::StandingTag::Core, &[]),
        host: systemprompt_api::routes::documents::files_host_handles(ctx),
    };
    Ok(systemprompt_files_entry::http::issues::IssuesState::build(
        &state,
    )?)
}
