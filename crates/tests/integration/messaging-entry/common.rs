use anyhow::Result;
use axum::body::Body;
use axum::http::{Response, StatusCode};
use http_body_util::BodyExt;

pub async fn body_to_string(resp: Response<Body>) -> Result<(StatusCode, String)> {
    let status = resp.status();
    let body = resp.into_body().collect().await?.to_bytes();
    Ok((status, String::from_utf8_lossy(&body).into_owned()))
}

pub fn messaging_admitted() -> std::sync::Arc<systemprompt_extension::Admitted> {
    let profile = systemprompt_test_fixtures::ensure_messaging_bootstrap()
        .boot
        .profile();
    std::sync::Arc::new(
        systemprompt::reference()
            .build(profile)
            .expect("the reference composition with Slack and Teams admits"),
    )
}

pub fn mount_set<C: Clone + Send + Sync + 'static>(
    ctx: &systemprompt_runtime::AppContext,
    set: &systemprompt_extension::router::RouteSet,
    clients: C,
) -> axum::Router {
    use systemprompt_extension::ExtensionContext;
    systemprompt_api::services::messaging::bind_dispatcher(ctx).expect("dispatcher slot seeded");
    let seed = ctx
        .extension_state()
        .with(ctx.services())
        .with(std::sync::Arc::clone(ctx.secrets()))
        .with(clients);
    let kernel = systemprompt_test_fixtures::offline_kernel(seed);
    set.materialize(&kernel.resolved())
        .expect("route set resolves against the seeded state")
        .into_iter()
        .fold(axum::Router::new(), axum::Router::merge)
}

pub fn slack_test_router(ctx: &systemprompt_runtime::AppContext) -> axum::Router {
    mount_set(
        ctx,
        &systemprompt_slack_entry::http::routes(),
        systemprompt_slack_entry::http::SlackClients::new(
            reqwest::Client::new(),
            systemprompt_client::GuardedClient::configured(
                &systemprompt_client::GuardedClientConfig::for_configured(),
            )
            .expect("loopback-permitting reply client"),
        ),
    )
}
