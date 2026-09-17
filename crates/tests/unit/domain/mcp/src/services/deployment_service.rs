//! Tests for `DeploymentService` accessors over a bootstrap-backed services
//! config containing one enabled and one disabled MCP server.

use systemprompt_mcp::services::DeploymentService;

use crate::harness::{
    ExternalServerSpec, bootstrap_with_services, config_with_servers, external_server_block,
};

fn seed_config() {
    let yaml = config_with_servers(&[
        external_server_block(&ExternalServerSpec {
            name: "dep_on",
            endpoint: "http://127.0.0.1:59999/mcp",
            oauth_required: false,
            enabled: true,
        }),
        external_server_block(&ExternalServerSpec {
            name: "dep_off",
            endpoint: "http://127.0.0.1:59998/mcp",
            oauth_required: true,
            enabled: false,
        }),
    ]);
    let _bootstrap = bootstrap_with_services(&yaml);
}

#[test]
fn deployment_accessors_resolve_configured_servers() {
    seed_config();

    let deployment = DeploymentService::get_deployment("dep_on").expect("deployment resolves");
    assert!(deployment.enabled);
    assert_eq!(deployment.binary, None);
    assert_eq!(deployment.port, None);
    assert_eq!(
        deployment.endpoint.as_deref(),
        Some("http://127.0.0.1:59999/mcp")
    );

    let mut enabled = DeploymentService::list_enabled_servers().expect("enabled list");
    enabled.sort();
    assert_eq!(enabled, vec!["dep_on".to_owned()]);

    assert!(DeploymentService::is_server_enabled("dep_on").unwrap());
    assert!(!DeploymentService::is_server_enabled("dep_off").unwrap());

    DeploymentService::validate_config().expect("config validates");
}

#[test]
fn deployment_accessors_error_for_unknown_server() {
    seed_config();

    assert!(DeploymentService::get_deployment("missing").is_err());
    assert!(DeploymentService::is_server_enabled("missing").is_err());
}
