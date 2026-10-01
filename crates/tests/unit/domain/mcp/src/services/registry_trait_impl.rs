//! Coverage for the async provider-trait impls on `RegistryService` and
//! `McpDeploymentProviderImpl`. The `Config`/`ConfigLoader` global is not
//! initialised in unit tests, so config-backed calls resolve deterministically
//! to their error arm; the trait wrappers still forward and map, which is what
//! we exercise. `protocol_version` is config-independent and asserted on
//! shape.

use systemprompt_identifiers::McpServerId;
use systemprompt_mcp::{McpDeploymentProviderImpl, RegistryService};
use systemprompt_models::mcp::{McpDeploymentProvider, McpRegistry};
use systemprompt_test_fixtures::fixture_user_id;

#[tokio::test]
async fn list_servers_is_reachable() {
    let registry = RegistryService::new(fixture_user_id());
    let _ = McpRegistry::list_servers(&registry).await;
}

#[tokio::test]
async fn server_exists_is_reachable() {
    let registry = RegistryService::new(fixture_user_id());
    let _ = McpRegistry::server_exists(
        &registry,
        &McpServerId::try_new("nonexistent-server").expect("valid"),
    )
    .await;
}

#[tokio::test]
async fn find_server_is_reachable() {
    let registry = RegistryService::new(fixture_user_id());
    let _ = McpRegistry::find_server(
        &registry,
        &McpServerId::try_new("nonexistent-server").expect("valid"),
    )
    .await;
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
