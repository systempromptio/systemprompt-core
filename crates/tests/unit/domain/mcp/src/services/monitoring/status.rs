//! Unit tests for ServiceStatus model

use systemprompt_core_mcp::services::monitoring::status::ServiceStatus;

fn create_test_status() -> ServiceStatus {
    ServiceStatus {
        state: "running".to_string(),
        pid: Some(1234),
        health: "healthy".to_string(),
        uptime_seconds: Some(3600),
        tools_count: 5,
        latency_ms: Some(100),
        auth_required: false,
    }
}

// ============================================================================
// ServiceStatus Field Access Tests
// ============================================================================

#[test]
fn test_service_status_fields() {
    let status = create_test_status();

    assert_eq!(status.state, "running");
    assert_eq!(status.pid, Some(1234));
    assert_eq!(status.health, "healthy");
    assert_eq!(status.uptime_seconds, Some(3600));
    assert_eq!(status.tools_count, 5);
    assert_eq!(status.latency_ms, Some(100));
    assert!(!status.auth_required);
}

#[test]
fn test_service_status_running() {
    let status = ServiceStatus {
        state: "running".to_string(),
        pid: Some(5678),
        health: "healthy".to_string(),
        uptime_seconds: Some(7200),
        tools_count: 10,
        latency_ms: Some(50),
        auth_required: false,
    };

    assert_eq!(status.state, "running");
    assert!(status.pid.is_some());
}

#[test]
fn test_service_status_stopped() {
    let status = ServiceStatus {
        state: "stopped".to_string(),
        pid: None,
        health: "unhealthy".to_string(),
        uptime_seconds: None,
        tools_count: 0,
        latency_ms: None,
        auth_required: false,
    };

    assert_eq!(status.state, "stopped");
    assert!(status.pid.is_none());
    assert!(status.uptime_seconds.is_none());
    assert!(status.latency_ms.is_none());
}

#[test]
fn test_service_status_error() {
    let status = ServiceStatus {
        state: "error".to_string(),
        pid: None,
        health: "unreachable".to_string(),
        uptime_seconds: None,
        tools_count: 0,
        latency_ms: None,
        auth_required: true,
    };

    assert_eq!(status.state, "error");
    assert_eq!(status.health, "unreachable");
}

#[test]
fn test_service_status_auth_required() {
    let status = ServiceStatus {
        state: "running".to_string(),
        pid: Some(9999),
        health: "healthy".to_string(),
        uptime_seconds: Some(1800),
        tools_count: 0,
        latency_ms: Some(200),
        auth_required: true,
    };

    assert!(status.auth_required);
}

#[test]
fn test_service_status_degraded() {
    let status = ServiceStatus {
        state: "running".to_string(),
        pid: Some(4567),
        health: "degraded".to_string(),
        uptime_seconds: Some(900),
        tools_count: 3,
        latency_ms: Some(2000),
        auth_required: false,
    };

    assert_eq!(status.health, "degraded");
    assert!(status.latency_ms.unwrap() > 1000);
}

#[test]
fn test_service_status_no_tools() {
    let status = ServiceStatus {
        state: "running".to_string(),
        pid: Some(1111),
        health: "healthy".to_string(),
        uptime_seconds: Some(600),
        tools_count: 0,
        latency_ms: Some(75),
        auth_required: true,
    };

    assert_eq!(status.tools_count, 0);
}

// ============================================================================
// ServiceStatus Clone Tests
// ============================================================================

#[test]
fn test_service_status_clone() {
    let status = create_test_status();
    let cloned = status.clone();

    assert_eq!(status.state, cloned.state);
    assert_eq!(status.pid, cloned.pid);
    assert_eq!(status.health, cloned.health);
    assert_eq!(status.tools_count, cloned.tools_count);
}

// ============================================================================
// ServiceStatus Debug Tests
// ============================================================================

#[test]
fn test_service_status_debug() {
    let status = create_test_status();
    let debug_str = format!("{:?}", status);

    assert!(debug_str.contains("ServiceStatus"));
    assert!(debug_str.contains("running"));
    assert!(debug_str.contains("healthy"));
}

// ============================================================================
// ServiceStatus Various States Tests
// ============================================================================

#[test]
fn test_service_status_starting() {
    let status = ServiceStatus {
        state: "starting".to_string(),
        pid: None,
        health: "unknown".to_string(),
        uptime_seconds: None,
        tools_count: 0,
        latency_ms: None,
        auth_required: false,
    };

    assert_eq!(status.state, "starting");
    assert_eq!(status.health, "unknown");
}

#[test]
fn test_service_status_long_uptime() {
    let status = ServiceStatus {
        state: "running".to_string(),
        pid: Some(2222),
        health: "healthy".to_string(),
        uptime_seconds: Some(86400 * 30), // 30 days
        tools_count: 15,
        latency_ms: Some(25),
        auth_required: false,
    };

    assert!(status.uptime_seconds.unwrap() > 86400);
}

#[test]
fn test_service_status_high_latency() {
    let status = ServiceStatus {
        state: "running".to_string(),
        pid: Some(3333),
        health: "degraded".to_string(),
        uptime_seconds: Some(100),
        tools_count: 2,
        latency_ms: Some(5000),
        auth_required: false,
    };

    assert!(status.latency_ms.unwrap() > 1000);
}
