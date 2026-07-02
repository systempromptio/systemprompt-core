//! Tests for `DeploymentService` accessors over a bootstrap-backed services
//! config containing one enabled and one disabled MCP server.

use systemprompt_mcp::services::DeploymentService;
use systemprompt_test_fixtures::ensure_test_bootstrap;

use crate::harness::{
    ExternalServerSpec, config_with_servers, external_server_block, write_services_config,
};

fn seed_config() {
    let bootstrap = ensure_test_bootstrap();
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
    write_services_config(bootstrap, &yaml);
}

#[test]
fn deployment_accessors_resolve_configured_servers() {
    seed_config();

    let deployment = DeploymentService::get_deployment("dep_on").expect("deployment resolves");
    assert!(deployment.enabled);
    assert_eq!(deployment.binary, "dep_on-bin");

    let mut enabled = DeploymentService::list_enabled_servers().expect("enabled list");
    enabled.sort();
    assert_eq!(enabled, vec!["dep_on".to_owned()]);

    assert_eq!(DeploymentService::get_server_port("dep_on").unwrap(), 0);
    assert!(DeploymentService::is_server_enabled("dep_on").unwrap());
    assert!(!DeploymentService::is_server_enabled("dep_off").unwrap());
    assert_eq!(
        DeploymentService::get_server_binary("dep_off").unwrap(),
        "dep_off-bin"
    );
    assert_eq!(
        DeploymentService::get_server_package("dep_on").unwrap(),
        "dep_on"
    );

    DeploymentService::validate_config().expect("config validates");
}

#[test]
fn deployment_accessors_error_for_unknown_server() {
    seed_config();

    assert!(DeploymentService::get_deployment("missing").is_err());
    assert!(DeploymentService::get_server_port("missing").is_err());
    assert!(DeploymentService::is_server_enabled("missing").is_err());
    assert!(DeploymentService::get_server_binary("missing").is_err());
}
