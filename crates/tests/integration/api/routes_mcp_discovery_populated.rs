//! MCP OAuth discovery documents against a services config that declares a
//! server.
//!
//! The shared bootstrap writes an empty services `config.yaml`, so no MCP
//! server is registered. This suite boots an isolated profile whose services
//! config declares one enabled and one disabled MCP server, the only way to
//! reach the per-server scopes the protected-resource document advertises.

use std::sync::OnceLock;

use systemprompt_test_fixtures::{TestBootstrap, init_isolated_bootstrap, test_app_context_on};
use tower::ServiceExt;

use super::common::{body_to_string, empty_get};

const ENABLED: &str = "fixture_enabled_mcp";
const DISABLED: &str = "fixture_disabled_mcp";

fn services_config() -> String {
    format!(
        r#"mcp_servers:
  {ENABLED}:
    server_type: external
    endpoint: http://127.0.0.1:5099/mcp
    enabled: true
    tool_policy: allow
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
    server_type: external
    endpoint: http://127.0.0.1:5098/mcp
    enabled: false
    tool_policy: allow
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

// The protected-resource discovery document derives its advertised scopes from
// the MCP server registry, via `get_mcp_server_scopes`. That lookup only produces
// scopes for a registered server whose `oauth.required` is set, so it is
// unreachable without a populated services config.
async fn protected_resource(path: &str) -> anyhow::Result<(u16, serde_json::Value)> {
    let b = boot();
    let ctx = test_app_context_on(&b).await;
    let (status, body) = body_to_string(
        super::common::with_config(
            systemprompt_api::routes::oauth::wellknown_routes(&ctx),
            &ctx,
        )
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
