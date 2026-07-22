//! Tests for `infra services status` output assembly: health labelling,
//! external MCP rows, and the running/stopped summary.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::HashMap;

use systemprompt_cli::infrastructure::services::status::{
    build_status_output, external_row, health_label, managed_health_label,
};
use systemprompt_mcp::{HealthStatus, McpServiceStatus};
use systemprompt_models::mcp::McpServerType;
use systemprompt_scheduler::{DesiredStatus, RuntimeStatus, ServiceType, VerifiedServiceState};

fn state(name: &str, service_type: ServiceType, runtime: RuntimeStatus) -> VerifiedServiceState {
    VerifiedServiceState::builder(
        name.to_string(),
        service_type,
        DesiredStatus::Enabled,
        runtime,
        8080,
    )
    .build()
}

fn mcp_status(name: &str, health: HealthStatus, endpoint: Option<&str>) -> McpServiceStatus {
    McpServiceStatus {
        name: name.to_string(),
        server_type: McpServerType::External,
        port: 0,
        endpoint: endpoint.map(str::to_string),
        health,
        pid: None,
        tools_count: 3,
        latency_ms: None,
        auth_required: false,
    }
}

#[test]
fn health_label_treats_degraded_as_ok_and_the_rest_as_degraded() {
    assert_eq!(health_label(HealthStatus::Healthy), "OK");
    assert_eq!(health_label(HealthStatus::Degraded), "OK");
    assert_eq!(health_label(HealthStatus::Unhealthy), "DEGRADED");
    assert_eq!(health_label(HealthStatus::Unknown), "DEGRADED");
}

#[test]
fn external_row_projects_remote_service_with_health_and_endpoint() {
    let row = external_row(&mcp_status(
        "github",
        HealthStatus::Healthy,
        Some("https://mcp.example.com"),
    ));
    assert_eq!(row.name, "github");
    assert_eq!(row.service_type, "mcp");
    assert_eq!(row.status, "remote");
    assert_eq!(row.port, 0);
    assert_eq!(row.action, "none");
    assert_eq!(row.health.as_deref(), Some("OK"));
    assert_eq!(row.endpoint.as_deref(), Some("https://mcp.example.com"));
}

#[test]
fn managed_health_label_uses_mcp_health_map_for_mcp_services() {
    let mut health = HashMap::new();
    health.insert("filesystem".to_string(), HealthStatus::Healthy);

    let known = state("filesystem", ServiceType::Mcp, RuntimeStatus::Running);
    assert_eq!(managed_health_label(&known, &health), "OK");

    let unknown = state("missing", ServiceType::Mcp, RuntimeStatus::Running);
    assert_eq!(managed_health_label(&unknown, &health), "DEGRADED");
}

#[test]
fn managed_health_label_uses_runtime_state_for_non_mcp_services() {
    let health = HashMap::new();
    let running = state("api", ServiceType::Api, RuntimeStatus::Running);
    assert_eq!(managed_health_label(&running, &health), "OK");

    let crashed = state("api", ServiceType::Api, RuntimeStatus::Crashed);
    assert_eq!(managed_health_label(&crashed, &health), "DEGRADED");
}

#[test]
fn build_status_output_counts_managed_and_external_running_services() {
    let states = vec![
        state("api", ServiceType::Api, RuntimeStatus::Running),
        state("agent", ServiceType::Agent, RuntimeStatus::Stopped),
    ];
    let external = vec![
        external_row(&mcp_status("github", HealthStatus::Healthy, None)),
        external_row(&mcp_status("broken", HealthStatus::Unhealthy, None)),
    ];

    let output = build_status_output(&states, &HashMap::new(), &external, false);

    assert_eq!(output.summary.total, 4);
    assert_eq!(output.summary.running, 2);
    assert_eq!(output.summary.stopped, 2);
    assert_eq!(output.services.len(), 4);
    assert!(output.services[0].health.is_none());
    assert_eq!(output.services[0].status, "running");
    assert_eq!(output.services[1].status, "stopped");
}

#[test]
fn build_status_output_attaches_health_labels_when_requested() {
    let states = vec![state("api", ServiceType::Api, RuntimeStatus::Running)];
    let output = build_status_output(&states, &HashMap::new(), &[], true);
    assert_eq!(output.services[0].health.as_deref(), Some("OK"));
}
