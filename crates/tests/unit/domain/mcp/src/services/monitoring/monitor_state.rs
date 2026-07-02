//! Tests for the health-monitor transition state machine and one bounded run
//! of the continuous monitor loop.

use std::time::Duration;

use systemprompt_mcp::services::monitoring::health::{
    HealthCheckDetails, HealthCheckResult, HealthStatus,
};
use systemprompt_mcp::services::monitoring::health_monitor::{
    HealthMonitorState, monitor_health_continuously,
};

use crate::harness::external_mcp_config;

fn result_with(status: HealthStatus, error: Option<&str>) -> HealthCheckResult {
    HealthCheckResult {
        status,
        connection_result: None,
        latency_ms: 5,
        details: HealthCheckDetails {
            service_name: "mon".to_owned(),
            tools_available: 0,
            requires_auth: false,
            validation_type: "test".to_owned(),
            error_message: error.map(ToOwned::to_owned),
            server_version: None,
        },
    }
}

#[test]
fn unhealthy_ticks_accumulate_failures() {
    let config = external_mcp_config("mon", "http://127.0.0.1:1/mcp");
    let mut state = HealthMonitorState::new();

    state.observe(&config, &result_with(HealthStatus::Unhealthy, Some("down")));
    state.observe(&config, &result_with(HealthStatus::Unhealthy, None));

    assert_eq!(state.failure_count(), 2);
    assert_eq!(state.previous_status(), Some(HealthStatus::Unhealthy));
}

#[test]
fn recovery_resets_failure_count() {
    let config = external_mcp_config("mon", "http://127.0.0.1:1/mcp");
    let mut state = HealthMonitorState::new();

    state.observe(&config, &result_with(HealthStatus::Unhealthy, Some("down")));
    state.observe(&config, &result_with(HealthStatus::Healthy, None));

    assert_eq!(state.failure_count(), 0);
    assert_eq!(state.previous_status(), Some(HealthStatus::Healthy));
}

#[test]
fn healthy_without_prior_failure_is_a_no_op() {
    let config = external_mcp_config("mon", "http://127.0.0.1:1/mcp");
    let mut state = HealthMonitorState::new();

    state.observe(&config, &result_with(HealthStatus::Healthy, None));

    assert_eq!(state.failure_count(), 0);
    assert_eq!(state.previous_status(), Some(HealthStatus::Healthy));
}

#[test]
fn degraded_after_healthy_keeps_failure_count() {
    let config = external_mcp_config("mon", "http://127.0.0.1:1/mcp");
    let mut state = HealthMonitorState::new();

    state.observe(&config, &result_with(HealthStatus::Healthy, None));
    state.observe(&config, &result_with(HealthStatus::Degraded, None));
    state.observe(&config, &result_with(HealthStatus::Unknown, None));

    assert_eq!(state.failure_count(), 0);
    assert_eq!(state.previous_status(), Some(HealthStatus::Unknown));
}

#[tokio::test]
async fn continuous_monitor_polls_until_cancelled() {
    let config = external_mcp_config("mon-loop", "http://127.0.0.1:1/mcp");

    let outcome = tokio::time::timeout(
        Duration::from_millis(400),
        monitor_health_continuously(&config, Duration::from_millis(50)),
    )
    .await;

    assert!(outcome.is_err(), "monitor loop only ends by cancellation");
}
