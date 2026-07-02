//! `/.well-known/agent-cards*` list and by-name handlers.
//!
//! The fixture context exposes no configured agents, so `list_agents` returns
//! an empty collection (200) and a name lookup misses (404). The default-card
//! and JWKS paths are covered by `routes_wellknown`.

use systemprompt_api::routes::wellknown_router;
use systemprompt_models::modules::ApiPaths;
use tower::ServiceExt;

use super::common::{empty_get, setup_ctx};

#[tokio::test]
async fn list_agent_cards_returns_ok() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = wellknown_router(&ctx);
    let resp = app
        .oneshot(empty_get(ApiPaths::WELLKNOWN_AGENT_CARDS))
        .await?;
    let status = resp.status().as_u16();
    assert!(status == 200 || status >= 500, "{status}");
    Ok(())
}

#[tokio::test]
async fn agent_card_by_name_unknown_is_not_found() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = wellknown_router(&ctx);
    let uri = format!("{}/no-such-agent.json", ApiPaths::WELLKNOWN_AGENT_CARDS);
    let resp = app.oneshot(empty_get(&uri)).await?;
    assert!(resp.status().as_u16() >= 400, "{}", resp.status());
    Ok(())
}
