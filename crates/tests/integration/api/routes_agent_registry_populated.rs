//! Agent-card listing against a registry that actually has an agent in it.
//!
//! The default bootstrap writes an empty services `config.yaml`, so
//! `handle_agent_registry` never gets past `AgentRegistry::new()` and the card
//! builder is never exercised. Opting into the messaging bootstrap seeds one
//! enabled agent, which lets the card builder run for real: the runtime status
//! it attaches is read from the `services` table, so the same agent yields
//! `NotStarted` with no row and the row's own status once one exists.

use axum::Extension;
use systemprompt_api::routes::agent::registry::create_mcp_extensions_from_config;
use systemprompt_api::routes::registry_router;
use systemprompt_database::DbPool;
use systemprompt_test_fixtures::{
    ensure_messaging_bootstrap, fixture_app_context, fixture_db_pool, seed_running_service,
    test_messaging_agent,
};
use tower::ServiceExt;

use super::common::{body_to_string, empty_get, request_context};

async fn setup() -> anyhow::Result<(DbPool, std::sync::Arc<systemprompt_runtime::AppContext>)> {
    let b = ensure_messaging_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let ctx = fixture_app_context(&pool, &b.database_url)?;
    Ok((pool, ctx))
}

async fn list_cards() -> anyhow::Result<serde_json::Value> {
    let (_pool, ctx) = setup().await?;
    let app = registry_router(&ctx).layer(Extension(request_context("registry_reader")));
    let resp = app.oneshot(empty_get("/")).await?;
    let (status, body) = body_to_string(resp).await?;
    assert_eq!(status.as_u16(), 200, "{body}");
    Ok(serde_json::from_str(&body)?)
}

fn card_for<'a>(listing: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
    listing["data"]
        .as_array()
        .or_else(|| listing.as_array())?
        .iter()
        .find(|c| c["name"].as_str() == Some(name) || c["displayName"].as_str() == Some(name))
}

#[tokio::test]
async fn the_registry_lists_a_card_for_the_configured_agent() -> anyhow::Result<()> {
    let listing = list_cards().await?;
    let items = listing["data"]
        .as_array()
        .or_else(|| listing.as_array())
        .cloned()
        .unwrap_or_default();

    assert!(
        !items.is_empty(),
        "a configured agent must produce a card: {listing}"
    );
    Ok(())
}

#[tokio::test]
async fn a_card_reports_the_status_of_the_agents_service_row() -> anyhow::Result<()> {
    let agent = test_messaging_agent();
    let (pool, ctx) = setup().await?;
    seed_running_service(&pool, agent, agent, 9250).await?;

    let app = registry_router(&ctx).layer(Extension(request_context("registry_reader")));
    let resp = app.oneshot(empty_get("/")).await?;
    let (status, body) = body_to_string(resp).await?;
    assert_eq!(status.as_u16(), 200, "{body}");

    let listing: serde_json::Value = serde_json::from_str(&body)?;
    let card = card_for(&listing, agent)
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    assert!(
        body.contains("running"),
        "the card must carry the service row's status: {card}"
    );
    Ok(())
}

#[test]
fn no_configured_mcp_servers_yields_no_extension() {
    assert!(
        create_mcp_extensions_from_config(&[], "http://api.test").is_empty(),
        "an agent with no MCP servers advertises no MCP extension"
    );
}

#[test]
fn configured_mcp_servers_are_advertised_with_gateway_relative_endpoints() {
    let extensions = create_mcp_extensions_from_config(
        &["alpha".to_owned(), "beta".to_owned()],
        "http://api.test",
    );

    assert_eq!(extensions.len(), 1, "servers collapse into one extension");
    let ext = &extensions[0];
    assert_eq!(ext.uri, "systemprompt:mcp-tools");
    assert_eq!(ext.required, Some(true));
    let params = ext.params.as_ref().expect("the extension carries params");
    let servers = params["servers"]
        .as_array()
        .expect("servers is an array of metadata");
    assert_eq!(servers.len(), 2);
    assert_eq!(
        servers[0]["endpoint"].as_str(),
        Some("http://api.test/api/v1/mcp/alpha/mcp"),
        "endpoints are synthesised from the API base url, never configured"
    );
    assert_eq!(servers[1]["name"].as_str(), Some("beta"));
    assert!(
        params["supported_protocols"]
            .as_array()
            .is_some_and(|p| !p.is_empty()),
        "the advertised protocol list must not be empty"
    );
}
