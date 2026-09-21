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

const TWO_AGENTS: &str = r#"agents:
  a_secondary:
    name: a_secondary
    port: 9322
    endpoint: /api/v1/agents/a_secondary/
    enabled: true
    dev_only: false
    is_primary: false
    default: false
    tags: []
    card:
      protocolVersion: 0.3.0
      name: a_secondary
      displayName: Secondary Agent
      description: Secondary registry fixture
      version: 1.0.0
      preferredTransport: JSONRPC
      capabilities: { streaming: true, pushNotifications: false, stateTransitionHistory: false }
      defaultInputModes: [text/plain]
      defaultOutputModes: [text/plain]
      supportsAuthenticatedExtendedCard: false
    metadata:
      systemPrompt: Secondary.
      mcpServers: { source: explicit }
      skills: { source: explicit }
      toolModelOverrides: {}
    oauth: { required: false, scopes: [], audience: a2a }
  z_default:
    name: z_default
    port: 9321
    endpoint: /api/v1/agents/z_default/
    enabled: true
    dev_only: false
    is_primary: true
    default: true
    tags: []
    card:
      protocolVersion: 0.3.0
      name: z_default
      displayName: Default Agent
      description: Default registry fixture
      version: 1.0.0
      preferredTransport: JSONRPC
      capabilities: { streaming: true, pushNotifications: false, stateTransitionHistory: false }
      defaultInputModes: [text/plain]
      defaultOutputModes: [text/plain]
      supportsAuthenticatedExtendedCard: false
    metadata:
      systemPrompt: Default.
      mcpServers: { source: explicit, include: [fixture_tools] }
      skills: { source: explicit }
      toolModelOverrides: {}
    oauth: { required: false, scopes: [], audience: a2a }
settings:
  agent_port_range: [9000, 9999]
  mcp_port_range: [5000, 5999]
"#;

fn service_status(card: &serde_json::Value) -> &serde_json::Value {
    card["capabilities"]["extensions"]
        .as_array()
        .expect("card extensions")
        .iter()
        .find(|extension| extension["uri"] == "systemprompt:service-status")
        .and_then(|extension| extension.get("params"))
        .expect("service status extension")
}

#[tokio::test]
async fn multi_agent_registry_sorts_default_first_and_retains_runtime_and_mcp_metadata()
-> anyhow::Result<()> {
    let boot = systemprompt_test_fixtures::init_isolated_bootstrap(
        "https://registry.example.test",
        TWO_AGENTS,
    );
    let pool = fixture_db_pool(&boot.database_url).await?;
    let ctx = fixture_app_context(&pool, &boot.database_url)?;
    seed_running_service(&pool, "a_secondary", "agent", 9322).await?;
    let app = registry_router(&ctx).layer(Extension(request_context("registry_order")));
    let response = app.oneshot(empty_get("/")).await?;
    let (status, body) = body_to_string(response).await?;
    assert_eq!(status.as_u16(), 200, "{body}");
    let listing: serde_json::Value = serde_json::from_str(&body)?;
    let cards = listing["data"].as_array().expect("registry cards");
    assert_eq!(cards.len(), 2, "{listing}");
    assert_eq!(cards[0]["name"], "z_default");
    assert_eq!(service_status(&cards[0])["default"], true);
    assert_eq!(service_status(&cards[0])["status"], "NotStarted");
    assert_eq!(service_status(&cards[1])["default"], false);
    assert_eq!(service_status(&cards[1])["status"], "running");
    assert_eq!(service_status(&cards[1])["port"], 9322);
    let mcp = cards[0]["capabilities"]["extensions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|extension| extension["uri"] == "systemprompt:mcp-tools")
        .expect("configured MCP metadata");
    assert_eq!(mcp["params"]["servers"][0]["name"], "fixture_tools");
    assert_eq!(
        mcp["params"]["servers"][0]["endpoint"],
        "http://127.0.0.1/api/v1/mcp/fixture_tools/mcp"
    );
    Ok(())
}

#[tokio::test]
async fn registry_preserves_configured_cards_with_unknown_status_when_database_is_unavailable()
-> anyhow::Result<()> {
    let boot = systemprompt_test_fixtures::init_isolated_bootstrap(
        "https://registry.example.test",
        TWO_AGENTS,
    );
    let pool = fixture_db_pool(&boot.database_url).await?;
    let ctx = fixture_app_context(&pool, &boot.database_url)?;
    pool.pool_arc()?.close().await;
    let app = registry_router(&ctx).layer(Extension(request_context("registry_db_fault")));
    let response = app.oneshot(empty_get("/")).await?;
    let (status, body) = body_to_string(response).await?;
    assert_eq!(status.as_u16(), 200, "{body}");
    let listing: serde_json::Value = serde_json::from_str(&body)?;
    let cards = listing["data"].as_array().expect("registry cards");
    assert_eq!(
        cards.len(),
        2,
        "database failure cannot erase configured agents"
    );
    assert!(
        cards
            .iter()
            .all(|card| service_status(card)["status"] == "Unknown"),
        "each card reports unavailable runtime state explicitly: {listing}"
    );
    assert_eq!(cards[0]["name"], "z_default");
    Ok(())
}
