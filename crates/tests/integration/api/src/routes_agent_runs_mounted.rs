//! `/api/v1/agent-runs` on the real router, mounted from the `agent_runs_api`
//! extension declaration: the user gate refuses a caller with no session on
//! every node role that serves the agent protocol, a signed-in caller reaches
//! the handlers, a profile that disables `agent_runs_api` mounts nothing, and
//! one that disables `agent_runs` while the routes stay enabled is refused at
//! boot.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AgentRunId, ExtensionId, JwtToken};
use systemprompt_manifest::profile::{NodeRole, RateLimitsConfig};
use systemprompt_models::modules::ApiPaths;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config,
    seed_bridge_credential, test_db_pool,
};
use tower::ServiceExt;

async fn node(role: NodeRole, disabled: &[ExtensionId]) -> Result<(DbPool, Router)> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.role = role;
    config.rate_limits = RateLimitsConfig::disabled();
    config.trusted_proxies = Vec::new();
    let ctx =
        fixture_app_context_with_extensions(config, super::common::reference_admitted(disabled)?)
            .await?;
    let router = setup_api_server(
        &ctx,
        systemprompt_api::services::server::routes::Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
        None,
        &Arc::new(ReadinessState::new()),
    )
    .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    Ok((pool, router))
}

async fn status(app: &Router, uri: &str, bearer: Option<&JwtToken>) -> Result<StatusCode> {
    let mut builder = Request::builder()
        .method("GET")
        .uri(uri)
        .header(header::HOST, "127.0.0.1")
        .header(header::USER_AGENT, "agent-runs-mounted-test/1.0");
    if let Some(token) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {}", token.as_str()));
    }
    let mut req = builder.body(Body::empty())?;
    req.extensions_mut()
        .insert(ConnectInfo("203.0.113.9:40000".parse::<SocketAddr>()?));
    Ok(app.clone().oneshot(req).await?.status())
}

fn run_uri() -> String {
    format!("{}/{}", ApiPaths::AGENT_RUNS, AgentRunId::generate())
}

#[tokio::test]
async fn the_user_gate_refuses_a_caller_without_a_session_on_every_agent_role() -> Result<()> {
    for role in [NodeRole::All, NodeRole::Api, NodeRole::Worker] {
        let (_pool, app) = node(role, &[]).await?;
        assert_eq!(
            status(&app, &run_uri(), None).await?,
            StatusCode::UNAUTHORIZED,
            "{role:?}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn a_signed_in_caller_passes_the_gate_to_the_handler() -> Result<()> {
    let (pool, app) = node(NodeRole::All, &[]).await?;
    let user = seed_bridge_credential(&pool, "agent-runs-mounted").await?;
    assert_eq!(
        status(&app, &run_uri(), Some(&user.jwt)).await?,
        StatusCode::NOT_FOUND
    );
    Ok(())
}

#[tokio::test]
async fn a_profile_that_disables_agent_runs_api_mounts_no_agent_run_route() -> Result<()> {
    for disabled in [vec!["agent_runs_api"], vec!["agent_runs_api", "agent_runs"]] {
        let ids: Vec<ExtensionId> = disabled.iter().map(|id| ExtensionId::new(*id)).collect();
        let (pool, app) = node(NodeRole::All, &ids).await?;
        let user = seed_bridge_credential(&pool, "agent-runs-off").await?;
        assert_eq!(
            status(&app, &run_uri(), None).await?,
            StatusCode::NOT_FOUND,
            "{disabled:?}"
        );
        assert_eq!(
            status(&app, &run_uri(), Some(&user.jwt)).await?,
            StatusCode::NOT_FOUND,
            "{disabled:?}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn disabling_agent_runs_while_its_routes_stay_enabled_is_refused_at_boot() -> Result<()> {
    let refused = node(NodeRole::All, &[ExtensionId::new("agent_runs")]).await;
    let message = match refused {
        Ok(_) => anyhow::bail!("boot accepted agent_runs_api without the run repository"),
        Err(error) => format!("{error:#}"),
    };
    assert!(
        message.contains("agent_runs_api requires")
            && message.contains("its provider agent_runs is disabled"),
        "{message}"
    );
    assert!(message.contains("AgentRunRepository"), "{message}");
    Ok(())
}

#[tokio::test]
async fn the_gateway_and_runner_roles_mount_no_agent_run_route() -> Result<()> {
    for role in [NodeRole::Gateway, NodeRole::Runner] {
        let (_pool, app) = node(role, &[]).await?;
        assert_eq!(
            status(&app, &run_uri(), None).await?,
            StatusCode::NOT_FOUND,
            "{role:?}"
        );
    }
    Ok(())
}
