//! The agent conversation route sets on the real router, mounted from the
//! `agent-http` extension's declarations:
//!
//! | path | anonymous | signed-in user | `agent-http` disabled |
//! |---|---|---|---|
//! | `GET /api/v1/core/{contexts,tasks,artifacts}` | 401 | 200 | 404 |
//! | `GET /api/v1/agents/registry` | 200 | 200 | the kernel agent proxy: 401 anonymous, 400 signed in |
//!
//! The registry set holds that one path: a path below it is the kernel agent
//! proxy's, never the registry's. A profile that disables `agent` while
//! `agent-http` stays enabled is refused at boot.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use systemprompt_agent_entry::extension::AGENT_HTTP_EXTENSION;
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ExtensionId, JwtToken};
use systemprompt_manifest::profile::{NodeRole, RateLimitsConfig};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config,
    seed_bridge_credential, test_db_pool,
};
use tower::ServiceExt;

const USER_PATHS: [&str; 3] = [
    "/api/v1/core/contexts",
    "/api/v1/core/tasks",
    "/api/v1/core/artifacts",
];
const REGISTRY: &str = "/api/v1/agents/registry";
const BELOW_REGISTRY: &str = "/api/v1/agents/registry/x";

async fn node(disabled: &[ExtensionId]) -> Result<(DbPool, Router)> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.role = NodeRole::All;
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

async fn get(app: &Router, uri: &str, bearer: Option<&JwtToken>) -> Result<(StatusCode, String)> {
    let mut builder = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header(header::HOST, "127.0.0.1");
    if let Some(token) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {}", token.as_str()));
    }
    let mut req = builder.body(Body::empty())?;
    req.extensions_mut()
        .insert(ConnectInfo("203.0.113.9:40000".parse::<SocketAddr>()?));
    let response = app.clone().oneshot(req).await?;
    let status = response.status();
    let body = response.into_body().collect().await?.to_bytes();
    Ok((status, String::from_utf8_lossy(&body).into_owned()))
}

#[tokio::test]
async fn the_user_sets_refuse_an_anonymous_caller_and_serve_a_signed_in_user() -> Result<()> {
    let (pool, app) = node(&[]).await?;
    let user = seed_bridge_credential(&pool, "agent-mounted").await?;
    for uri in USER_PATHS {
        let (anonymous, body) = get(&app, uri, None).await?;
        assert_eq!(anonymous, StatusCode::UNAUTHORIZED, "{uri}: {body}");
        let (signed_in, body) = get(&app, uri, Some(&user.jwt)).await?;
        assert_eq!(signed_in, StatusCode::OK, "{uri}: {body}");
    }
    Ok(())
}

#[tokio::test]
async fn the_registry_serves_an_anonymous_caller_at_its_one_path() -> Result<()> {
    let (_pool, app) = node(&[]).await?;
    let (status, body) = get(&app, REGISTRY, None).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (below, body) = get(&app, BELOW_REGISTRY, None).await?;
    assert_ne!(
        below,
        StatusCode::OK,
        "a path below the registry is the kernel proxy's: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn disabling_agent_http_unmounts_every_set() -> Result<()> {
    let (pool, app) = node(&[ExtensionId::new(AGENT_HTTP_EXTENSION)]).await?;
    let user = seed_bridge_credential(&pool, "agent-off").await?;
    for uri in USER_PATHS {
        let (status, body) = get(&app, uri, Some(&user.jwt)).await?;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}: {body}");
    }
    let (status, body) = get(&app, REGISTRY, None).await?;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the reserved registry path keeps the authenticated gate: {body}"
    );
    assert!(body.contains("Missing Authorization header"), "{body}");
    let (authed, body) = get(&app, REGISTRY, Some(&user.jwt)).await?;
    assert_eq!(
        authed,
        StatusCode::NOT_FOUND,
        "the reserved registry path answers 404 ahead of the agent proxy: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn disabling_agent_while_its_routes_stay_enabled_is_refused_at_boot() -> Result<()> {
    let refused = node(&[ExtensionId::new("agent")]).await;
    let message = match refused {
        Ok(_) => anyhow::bail!("boot accepted agent-http without the agent extension"),
        Err(error) => format!("{error:#}"),
    };
    assert!(
        message.contains("its provider agent is disabled"),
        "{message}"
    );
    Ok(())
}
