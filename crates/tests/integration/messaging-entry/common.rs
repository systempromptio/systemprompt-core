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

pub fn messaging_members() -> std::sync::Arc<systemprompt_extension::Admitted> {
    let mut members = systemprompt_test_fixtures::fixture_core_members();
    members.push(Box::new(
        systemprompt_agent_entry::extension::AgentHttpExtension,
    ));
    members.push(Box::new(systemprompt_slack_entry::SlackExtension));
    members.push(Box::new(systemprompt_teams_entry::TeamsExtension));
    systemprompt_test_fixtures::fixture_admitted_of(members)
}

pub async fn messaging_context(
    database_url: &str,
) -> std::sync::Arc<systemprompt_runtime::AppContext> {
    systemprompt_test_fixtures::fixture_app_context_with_extensions(
        systemprompt_test_fixtures::fixture_config(database_url),
        messaging_members(),
    )
    .await
    .unwrap_or_else(|e| panic!("the fixture context with Slack and Teams builds: {e:#}"))
}

pub async fn messaging_context_on(
    bootstrap: &systemprompt_test_fixtures::TestBootstrap,
) -> std::sync::Arc<systemprompt_runtime::AppContext> {
    systemprompt_test_fixtures::test_app_context_on_with(bootstrap, messaging_members()).await
}

pub fn bind_dispatcher_once(ctx: &systemprompt_runtime::AppContext) {
    match systemprompt_api::services::agent_dispatch::bind_dispatchers(ctx) {
        Ok(())
        | Err(systemprompt_api::services::agent_dispatch::DispatcherBindError::BindersTaken) => {},
        Err(error) => panic!("the dispatcher cells bind: {error}"),
    }
}

pub fn mount_set<C: Clone + Send + Sync + 'static>(
    ctx: &systemprompt_runtime::AppContext,
    set: &systemprompt_extension::router::RouteSet,
    clients: C,
) -> axum::Router {
    bind_dispatcher_once(ctx);
    let seed = ctx
        .kernel()
        .state()
        .clone()
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
