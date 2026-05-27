//! `/.well-known/*` endpoints.

use systemprompt_api::routes::wellknown_router;
use tower::ServiceExt;

use super::common::{empty_get, setup_ctx};

#[tokio::test]
async fn jwks_route_executes() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = wellknown_router(&ctx);

    let resp = app.oneshot(empty_get("/.well-known/jwks.json")).await?;
    let status = resp.status();
    assert!(status.as_u16() == 200 || status.as_u16() >= 500, "{status}");
    Ok(())
}

#[tokio::test]
async fn default_agent_card_returns_error_without_profile() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = wellknown_router(&ctx);
    let resp = app
        .oneshot(empty_get("/.well-known/agent-card.json"))
        .await?;
    assert!(resp.status().as_u16() >= 400);
    Ok(())
}

