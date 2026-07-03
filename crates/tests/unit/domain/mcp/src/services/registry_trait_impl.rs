//! Coverage for the async provider-trait impls on `RegistryService` and
//! `McpDeploymentProviderImpl`. The `Config`/`ConfigLoader` global is not
//! initialised in unit tests, so config-backed calls resolve deterministically
//! to their error arm; the trait wrappers still forward and map, which is what
//! we exercise. `load_tools_for_servers(&[])` and `protocol_version` are
//! config-independent and asserted on shape.

use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId};
use systemprompt_mcp::{McpDeploymentProviderImpl, RegistryService};
use systemprompt_models::RequestContext;
use systemprompt_models::mcp::{McpDeploymentProvider, McpRegistry, McpToolProvider};
use systemprompt_test_fixtures::fixture_user_id;

fn ctx() -> RequestContext {
    RequestContext::new(
        SessionId::new("s-trait"),
        TraceId::new("t-trait"),
        ContextId::new("00000000-0000-4000-8000-0000000000aa"),
        AgentName::new("agent-trait"),
    )
}

#[tokio::test]
async fn list_servers_is_reachable() {
    let registry = RegistryService::new(fixture_user_id());
    let _ = McpRegistry::list_servers(&registry).await;
}

#[tokio::test]
async fn server_exists_is_reachable() {
    let registry = RegistryService::new(fixture_user_id());
    let _ = McpRegistry::server_exists(&registry, "nonexistent-server").await;
}

#[tokio::test]
async fn find_server_is_reachable() {
    let registry = RegistryService::new(fixture_user_id());
    let _ = McpRegistry::find_server(&registry, "nonexistent-server").await;
}

#[tokio::test]
async fn load_tools_for_empty_server_list_is_empty_map() {
    let registry = RegistryService::new(fixture_user_id());
    let tools = McpToolProvider::load_tools_for_servers(&registry, &[], &ctx())
        .await
        .expect("empty server list needs no config and yields an empty map");
    assert!(tools.is_empty());
}

#[tokio::test]
async fn load_tools_for_unknown_servers_skips_them() {
    let registry = RegistryService::new(fixture_user_id());
    let names = vec!["no-such-server-a".to_owned(), "no-such-server-b".to_owned()];
    let tools = McpToolProvider::load_tools_for_servers(&registry, &names, &ctx())
        .await
        .expect("unresolvable servers are skipped, not fatal");
    assert!(
        tools.is_empty(),
        "servers that fail to resolve contribute no tools"
    );
}

#[test]
fn deployment_provider_reports_protocol_version() {
    let provider = McpDeploymentProviderImpl;
    assert!(
        !provider.protocol_version().is_empty(),
        "the MCP protocol version string is non-empty"
    );
}

#[tokio::test]
async fn deployment_provider_load_config_is_reachable() {
    let provider = McpDeploymentProviderImpl;
    let _ = provider.load_config().await;
}
