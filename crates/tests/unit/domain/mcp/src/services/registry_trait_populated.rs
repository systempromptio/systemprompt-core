//! Provider-trait impls on `RegistryService` over a POPULATED registry: the
//! Some/success arms that the config-error smoke tests never reach.

use systemprompt_identifiers::McpServerId;
use systemprompt_mcp::RegistryService;
use systemprompt_models::mcp::McpRegistry;
use systemprompt_test_fixtures::fixture_user_id;
use systemprompt_traits::McpRegistryProvider;
use wiremock::MockServer;

use crate::harness::{
    ExternalServerSpec, bootstrap_with_services, config_with_servers, default_tools_json,
    external_server_block, mount_mcp_endpoint,
};

async fn populated_registry() -> (RegistryService, McpServerId, MockServer) {
    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;

    let name = McpServerId::try_new(format!("rtp_{}", uuid::Uuid::new_v4().simple()))
        .expect("valid McpServerId");
    let _bootstrap = bootstrap_with_services(&config_with_servers(&[external_server_block(
        &ExternalServerSpec {
            name: name.as_str(),
            endpoint: &format!("{}/mcp", mock.uri()),
            oauth_required: false,
            enabled: true,
        },
    )]));
    (
        RegistryService::new(
            fixture_user_id(),
            crate::harness::services_state().view(),
            crate::harness::config(),
        ),
        name,
        mock,
    )
}

#[tokio::test]
async fn registry_find_server_returns_state_for_known_server() {
    let (registry, name, _mock) = populated_registry().await;

    let state = McpRegistry::find_server(&registry, &name)
        .await
        .expect("registry reachable")
        .expect("server known");
    assert_eq!(state.name, name);

    let servers = McpRegistry::list_servers(&registry)
        .await
        .expect("list servers");
    assert!(servers.contains(&name));
    assert!(
        McpRegistry::server_exists(&registry, &name)
            .await
            .expect("exists check")
    );
}

#[tokio::test]
async fn registry_provider_reports_server_info_and_enabled_set() {
    let (registry, name, _mock) = populated_registry().await;

    let info = McpRegistryProvider::get_server(&registry, name.as_str())
        .await
        .expect("server info");
    assert_eq!(info.name, name.as_str());
    assert!(info.enabled);
    assert!(!info.oauth.required);
    assert_eq!(info.oauth.audience, "mcp");

    let enabled = registry
        .list_enabled_servers()
        .await
        .expect("enabled servers");
    assert!(enabled.iter().any(|s| s.name == name.as_str()));
}

#[tokio::test]
async fn registry_provider_reports_an_unknown_server_as_not_found() {
    let (registry, _name, _mock) = populated_registry().await;

    let err = McpRegistryProvider::get_server(&registry, "rtp_no_such_server")
        .await
        .expect_err("an unregistered server is not resolved");
    assert!(
        matches!(err, systemprompt_traits::RegistryError::NotFound(_)),
        "a missing server is NotFound, not an outage: {err:?}"
    );
    assert!(
        !McpRegistryProvider::server_exists(&registry, "rtp_no_such_server")
            .await
            .expect("a missing server is not an error")
    );
}

#[tokio::test]
async fn registry_accessors_and_partitions_name_the_external_server() {
    let (registry, name, _mock) = populated_registry().await;

    assert_eq!(registry.owner(), &fixture_user_id());
    assert!(
        registry
            .services()
            .config()
            .mcp_servers
            .contains_key(name.as_str())
    );
    assert!(
        registry
            .deployments()
            .list_enabled_servers()
            .contains(&name.as_str().to_owned())
    );

    let external = registry.get_external_servers().expect("external");
    assert!(external.iter().any(|s| s.name == name.as_str()));
    let managed = registry.get_managed_servers().expect("managed");
    assert!(managed.iter().all(|s| s.name != name.as_str()));
    registry.reload().expect("reload validates");

    let found = registry.get_server(name.as_str()).expect("server");
    assert_eq!(
        found.description,
        format!("{} MCP Server", found.display_name)
    );
}

#[tokio::test]
async fn registry_get_server_of_an_unknown_name_is_not_found() {
    let (registry, _name, _mock) = populated_registry().await;

    let err = registry
        .get_server("rtp_no_such_server")
        .expect_err("unknown");
    assert!(
        matches!(err, systemprompt_mcp::McpDomainError::ServerNotFound(_)),
        "{err:?}"
    );
    let converted = systemprompt_models::errors::McpRegistryError::from(err);
    assert!(
        matches!(
            converted,
            systemprompt_models::errors::McpRegistryError::NotFound(_)
        ),
        "{converted:?}"
    );
}
