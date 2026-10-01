//! Tests for `display_service_status` with non-empty data, covering the
//! running/error counting branches and the per-server iteration loop.

use std::path::PathBuf;
use systemprompt_mcp::services::monitoring::health::HealthStatus;
use systemprompt_mcp::services::monitoring::status::{McpServiceStatus, display_service_status};
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::mcp::deployment::{McpServerType, OAuthRequirement};
use systemprompt_models::mcp::server::McpServerConfig;
use systemprompt_test_fixtures::fixture_user_id;

fn make_config(name: &str) -> McpServerConfig {
    McpServerConfig {
        name: name.to_owned(),
        owner: fixture_user_id(),
        server_type: McpServerType::Internal,
        binary: Some("bin".to_owned()),
        enabled: true,
        display_in_web: false,
        port: Some(0),
        crate_path: PathBuf::from("."),
        display_name: name.to_owned(),
        description: name.to_owned(),
        capabilities: vec![],
        schemas: vec![],
        oauth: OAuthRequirement {
            required: false,
            scopes: vec![],
            audience: JwtAudience::Mcp,
            client_id: None,
            ema: false,
        },
        tools: Default::default(),
        model_config: None,
        env_vars: vec![],
        version: "0.1.0".to_owned(),
        host: "127.0.0.1".to_owned(),
        module_name: "mcp".to_owned(),
        protocol: "mcp".to_owned(),
        remote_endpoint: String::new(),
        external_auth: None,
        headers: Default::default(),
    }
}

fn status(name: &str, health: HealthStatus) -> McpServiceStatus {
    McpServiceStatus {
        health,
        ..McpServiceStatus::unreachable(&make_config(name))
    }
}

#[test]
fn display_service_status_single_running() {
    display_service_status(&[status("svc-a", HealthStatus::Healthy)]);
}

#[test]
fn display_service_status_mixed_states() {
    display_service_status(&[
        status("alpha", HealthStatus::Healthy),
        status("beta", HealthStatus::Unknown),
        status("gamma", HealthStatus::Unhealthy),
    ]);
}

#[test]
fn display_service_status_all_error() {
    display_service_status(&[
        status("err1", HealthStatus::Unknown),
        status("err2", HealthStatus::Unknown),
    ]);
}

#[test]
fn display_service_status_empty_servers() {
    display_service_status(&[]);
}

#[test]
fn display_service_status_many_running() {
    let statuses: Vec<_> = ["s1", "s2", "s3", "s4", "s5"]
        .iter()
        .map(|n| status(n, HealthStatus::Healthy))
        .collect();
    display_service_status(&statuses);
}
