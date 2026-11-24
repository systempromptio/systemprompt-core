//! The `agent-http` member's surface as the full server mounts it.
//!
//! | route | anonymous | signed-in user | `agent-http` disabled (signed in) |
//! |---|---|---|---|
//! | `POST /api/v1/webhook/broadcast` | 401 | served | 404 |
//! | `GET /api/v1/stream/contexts` | 401 | served | 404 |
//! | `GET /.well-known/agent-cards` | 200 | 200 | 404 |
//! | `GET /api/v1/agents/registry` | 200 | 200 | 404 (401 anonymous) |
//!
//! The webhook is limited under its own `webhook` scope (200 per second per
//! caller, times the profile burst multiplier): past the budget it refuses
//! with 429.
//!
//! With the member disabled the exact registry reservation answers 404 ahead
//! of the A2A proxy's `/{service_name}` route, so it never reaches the proxy's
//! request parsing.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, StatusCode, header};
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ExtensionId, JwtToken};
use systemprompt_manifest::profile::{NodeRole, RateLimitsConfig};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config,
    seed_bridge_credential, test_db_pool,
};
use tower::ServiceExt;

const WEBHOOK: &str = "/api/v1/webhook/broadcast";
const STREAM: &str = "/api/v1/stream/contexts";
const CARDS: &str = "/.well-known/agent-cards";
const REGISTRY: &str = "/api/v1/agents/registry";

async fn node(disabled: &[ExtensionId]) -> Result<(DbPool, Router)> {
    node_limited(disabled, RateLimitsConfig::disabled()).await
}

async fn node_limited(
    disabled: &[ExtensionId],
    rate_limits: RateLimitsConfig,
) -> Result<(DbPool, Router)> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.role = NodeRole::All;
    config.rate_limits = rate_limits;
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

async fn status(
    app: &Router,
    method: Method,
    uri: &str,
    bearer: Option<&JwtToken>,
) -> Result<StatusCode> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "127.0.0.1")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {}", token.as_str()));
    }
    let mut request = builder.body(Body::from("{}"))?;
    request
        .extensions_mut()
        .insert(ConnectInfo("203.0.113.11:40000".parse::<SocketAddr>()?));
    Ok(app.clone().oneshot(request).await?.status())
}

fn agent_http() -> ExtensionId {
    ExtensionId::new("agent-http")
}

#[tokio::test]
async fn the_admitted_member_serves_its_routes_behind_their_gates() -> Result<()> {
    let (pool, app) = node(&[]).await?;
    assert_eq!(
        status(&app, Method::POST, WEBHOOK, None).await?,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        status(&app, Method::GET, STREAM, None).await?,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        status(&app, Method::GET, CARDS, None).await?,
        StatusCode::OK
    );
    assert_eq!(
        status(&app, Method::GET, REGISTRY, None).await?,
        StatusCode::OK
    );

    let user = seed_bridge_credential(&pool, "agent-http-mounted").await?;
    let webhook = status(&app, Method::POST, WEBHOOK, Some(&user.jwt)).await?;
    assert_ne!(webhook, StatusCode::NOT_FOUND, "the webhook is mounted");
    assert_ne!(
        webhook,
        StatusCode::UNAUTHORIZED,
        "a signed-in user passes the gate"
    );
    Ok(())
}

#[tokio::test]
async fn a_disabled_agent_http_member_mounts_none_of_its_routes() -> Result<()> {
    let (pool, app) = node(&[agent_http()]).await?;
    let user = seed_bridge_credential(&pool, "agent-http-disabled").await?;
    assert_eq!(
        status(&app, Method::POST, WEBHOOK, Some(&user.jwt)).await?,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status(&app, Method::GET, STREAM, Some(&user.jwt)).await?,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status(&app, Method::GET, CARDS, None).await?,
        StatusCode::NOT_FOUND
    );
    Ok(())
}

#[tokio::test]
async fn a_disabled_agent_http_member_answers_the_registry_404_ahead_of_the_proxy() -> Result<()> {
    let (pool, app) = node(&[agent_http()]).await?;
    let user = seed_bridge_credential(&pool, "agent-http-registry").await?;
    assert_eq!(
        status(&app, Method::GET, REGISTRY, Some(&user.jwt)).await?,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status(&app, Method::POST, REGISTRY, Some(&user.jwt)).await?,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status(&app, Method::GET, REGISTRY, None).await?,
        StatusCode::UNAUTHORIZED,
        "the proxy's authenticated gate still runs first"
    );
    Ok(())
}

#[tokio::test]
async fn the_webhook_refuses_past_its_rate_budget() -> Result<()> {
    let (pool, app) = node_limited(
        &[],
        RateLimitsConfig {
            burst_multiplier: 1,
            disabled: false,
            ..RateLimitsConfig::testing()
        },
    )
    .await?;
    let user = seed_bridge_credential(&pool, "agent-http-webhook-budget").await?;
    let first = status(&app, Method::POST, WEBHOOK, Some(&user.jwt)).await?;
    assert_ne!(
        first,
        StatusCode::TOO_MANY_REQUESTS,
        "the first call is within budget"
    );
    let burst = futures::future::join_all(
        (0..300).map(|_| status(&app, Method::POST, WEBHOOK, Some(&user.jwt))),
    )
    .await;
    let mut refused = 0_usize;
    for outcome in burst {
        if outcome? == StatusCode::TOO_MANY_REQUESTS {
            refused += 1;
        }
    }
    assert!(
        refused > 0,
        "the webhook scope refuses a caller past its 200-request budget"
    );
    Ok(())
}
