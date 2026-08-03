//! MCP registry listing against a services config that declares a server.
//!
//! The shared bootstrap writes an empty services `config.yaml`, so
//! `get_enabled_servers()` returns nothing and the per-server projection in
//! `handle_mcp_registry` never runs. This suite boots an isolated profile whose
//! services config declares one enabled and one disabled MCP server, which is
//! also the only way to check that the advertised endpoint is synthesised
//! rather than taken from the config.

use std::sync::OnceLock;

use axum::Router;
use systemprompt_api::routes::mcp_registry_router;
use systemprompt_test_fixtures::{
    TestBootstrap, fixture_app_context, fixture_db_pool, init_isolated_bootstrap,
};
use tower::ServiceExt;

use super::common::{body_to_string, empty_get};

const ENABLED: &str = "fixture_enabled_mcp";
const DISABLED: &str = "fixture_disabled_mcp";

fn services_config() -> String {
    format!(
        r#"mcp_servers:
  {ENABLED}:
    type: external
    binary: ""
    remote_endpoint: http://127.0.0.1:5099/mcp
    package: fixture
    port: 5099
    enabled: true
    display_in_web: true
    version: "2.1.0"
    description: An enabled fixture MCP server
    oauth:
      required: true
      scopes:
        - user
        - mcp
      audience: mcp
      client_id: null
  {DISABLED}:
    type: external
    binary: ""
    remote_endpoint: http://127.0.0.1:5098/mcp
    package: fixture
    port: 5098
    enabled: false
    display_in_web: false
    oauth:
      required: false
      scopes: []
      audience: mcp
      client_id: null
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
"#
    )
}

// The bootstrap owns the tempdir holding the services config, and the handler
// reads that file on every request — so it has to outlive the router.
static BOOT: OnceLock<TestBootstrap> = OnceLock::new();

fn boot() -> &'static TestBootstrap {
    BOOT.get_or_init(|| init_isolated_bootstrap("http://127.0.0.1", &services_config()))
}

async fn app() -> anyhow::Result<Router> {
    let b = boot();
    let pool = fixture_db_pool(&b.database_url).await?;
    let ctx = fixture_app_context(&pool, &b.database_url)?;
    Ok(mcp_registry_router(&ctx))
}

async fn listing() -> anyhow::Result<serde_json::Value> {
    let (status, body) = body_to_string(app().await?.oneshot(empty_get("/")).await?).await?;
    assert_eq!(status.as_u16(), 200, "{body}");
    Ok(serde_json::from_str(&body)?)
}

fn entries(listing: &serde_json::Value) -> Vec<serde_json::Value> {
    listing["data"]
        .as_array()
        .or_else(|| listing.as_array())
        .cloned()
        .unwrap_or_default()
}

#[tokio::test]
async fn only_enabled_servers_are_listed() -> anyhow::Result<()> {
    let listing = listing().await?;
    let names: Vec<String> = entries(&listing)
        .iter()
        .filter_map(|e| e["name"].as_str().map(str::to_owned))
        .collect();

    assert!(names.iter().any(|n| n == ENABLED), "{listing}");
    assert!(
        !names.iter().any(|n| n == DISABLED),
        "a disabled server must not be advertised in the registry: {listing}"
    );
    Ok(())
}

#[tokio::test]
async fn a_listed_server_carries_its_oauth_requirement_and_scopes() -> anyhow::Result<()> {
    let listing = listing().await?;
    let entry = entries(&listing)
        .into_iter()
        .find(|e| e["name"].as_str() == Some(ENABLED))
        .expect("the enabled server is listed");

    assert_eq!(entry["oauth_required"].as_bool(), Some(true), "{entry}");
    let scopes: Vec<&str> = entry["oauth_scopes"]
        .as_array()
        .expect("scopes are an array")
        .iter()
        .filter_map(|s| s.as_str())
        .collect();
    assert!(scopes.contains(&"user"), "{entry}");
    assert!(scopes.contains(&"mcp"), "{entry}");
    assert_eq!(entry["status"].as_str(), Some("enabled"), "{entry}");
    assert_eq!(entry["port"].as_u64(), Some(5099), "{entry}");
    Ok(())
}

#[tokio::test]
async fn the_advertised_endpoint_is_synthesised_not_taken_from_config() -> anyhow::Result<()> {
    let listing = listing().await?;
    let entry = entries(&listing)
        .into_iter()
        .find(|e| e["name"].as_str() == Some(ENABLED))
        .expect("the enabled server is listed");

    let endpoint = entry["endpoint"].as_str().unwrap_or_default();
    assert!(
        endpoint.contains(ENABLED) && endpoint.contains("/mcp"),
        "the endpoint is derived from the server name: {entry}"
    );
    assert!(
        !endpoint.contains("127.0.0.1:5099"),
        "the config's remote_endpoint must never leak into the public registry: {entry}"
    );
    Ok(())
}

// The protected-resource discovery document derives its advertised scopes from
// the same registry, via `get_mcp_server_scopes`. That lookup only produces
// scopes for a registered server whose `oauth.required` is set, so it is
// unreachable without a populated services config.
async fn protected_resource(path: &str) -> anyhow::Result<(u16, serde_json::Value)> {
    let b = boot();
    let pool = fixture_db_pool(&b.database_url).await?;
    let ctx = fixture_app_context(&pool, &b.database_url)?;
    let (status, body) = body_to_string(
        systemprompt_api::routes::oauth::wellknown_routes(&ctx)
            .oneshot(empty_get(path))
            .await?,
    )
    .await?;
    Ok((status.as_u16(), serde_json::from_str(&body)?))
}

#[tokio::test]
async fn the_discovery_document_advertises_the_servers_configured_scopes() -> anyhow::Result<()> {
    let (status, doc) = protected_resource(&format!(
        "/.well-known/oauth-protected-resource/api/v1/mcp/{ENABLED}/mcp"
    ))
    .await?;

    assert_eq!(status, 200, "{doc}");
    let scopes: Vec<&str> = doc["scopes_supported"]
        .as_array()
        .expect("scopes_supported is an array")
        .iter()
        .filter_map(|s| s.as_str())
        .collect();
    assert!(scopes.contains(&"user") && scopes.contains(&"mcp"), "{doc}");
    assert!(
        doc["resource"]
            .as_str()
            .is_some_and(|r| r.contains(ENABLED)),
        "the document must name the resource it describes: {doc}"
    );
    Ok(())
}

#[tokio::test]
async fn a_server_with_no_oauth_requirement_falls_back_to_the_default_scope() -> anyhow::Result<()>
{
    let (status, doc) = protected_resource(&format!(
        "/.well-known/oauth-protected-resource/api/v1/mcp/{DISABLED}/mcp"
    ))
    .await?;

    assert_eq!(status, 200, "{doc}");
    let scopes: Vec<&str> = doc["scopes_supported"]
        .as_array()
        .expect("scopes_supported is an array")
        .iter()
        .filter_map(|s| s.as_str())
        .collect();
    assert_eq!(
        scopes,
        vec!["user"],
        "a server that requires no oauth must not advertise privileged scopes: {doc}"
    );
    Ok(())
}

#[tokio::test]
async fn a_path_that_does_not_name_an_mcp_server_serves_the_generic_document() -> anyhow::Result<()>
{
    let (status, doc) =
        protected_resource("/.well-known/oauth-protected-resource/something/else").await?;

    assert_eq!(status, 200, "{doc}");
    assert!(
        !doc["resource"]
            .as_str()
            .unwrap_or_default()
            .contains("/mcp/"),
        "an unrecognised path must fall back to the server-wide document: {doc}"
    );
    Ok(())
}

#[tokio::test]
async fn a_nested_service_name_is_not_treated_as_a_server() -> anyhow::Result<()> {
    // The name is filtered for embedded slashes so a crafted path cannot make
    // the document advertise a resource the registry never declared.
    let (status, doc) =
        protected_resource("/.well-known/oauth-protected-resource/api/v1/mcp/a/b/mcp").await?;

    assert_eq!(status, 200, "{doc}");
    assert!(
        !doc["resource"].as_str().unwrap_or_default().contains("a/b"),
        "{doc}"
    );
    Ok(())
}
