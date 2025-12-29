//! Unit tests for MCP events

use systemprompt_core_mcp::services::orchestrator::McpEvent;

// ============================================================================
// McpEvent service_name Tests
// ============================================================================

#[test]
fn test_mcp_event_service_start_requested_service_name() {
    let event = McpEvent::ServiceStartRequested {
        service_name: "test-service".to_string(),
    };
    assert_eq!(event.service_name(), "test-service");
}

#[test]
fn test_mcp_event_service_start_completed_service_name() {
    let event = McpEvent::ServiceStartCompleted {
        service_name: "test-service".to_string(),
        success: true,
        pid: Some(1234),
        port: Some(8080),
        error: None,
        duration_ms: 100,
    };
    assert_eq!(event.service_name(), "test-service");
}

#[test]
fn test_mcp_event_service_started_service_name() {
    let event = McpEvent::ServiceStarted {
        service_name: "test-service".to_string(),
        process_id: 1234,
        port: 8080,
    };
    assert_eq!(event.service_name(), "test-service");
}

#[test]
fn test_mcp_event_service_failed_service_name() {
    let event = McpEvent::ServiceFailed {
        service_name: "test-service".to_string(),
        error: "Connection failed".to_string(),
    };
    assert_eq!(event.service_name(), "test-service");
}

#[test]
fn test_mcp_event_service_stopped_service_name() {
    let event = McpEvent::ServiceStopped {
        service_name: "test-service".to_string(),
        exit_code: Some(0),
    };
    assert_eq!(event.service_name(), "test-service");
}

#[test]
fn test_mcp_event_health_check_failed_service_name() {
    let event = McpEvent::HealthCheckFailed {
        service_name: "test-service".to_string(),
        reason: "Timeout".to_string(),
    };
    assert_eq!(event.service_name(), "test-service");
}

#[test]
fn test_mcp_event_schema_updated_service_name() {
    let event = McpEvent::SchemaUpdated {
        service_name: "test-service".to_string(),
        tool_count: 5,
    };
    assert_eq!(event.service_name(), "test-service");
}

#[test]
fn test_mcp_event_service_restart_requested_service_name() {
    let event = McpEvent::ServiceRestartRequested {
        service_name: "test-service".to_string(),
        reason: "Manual restart".to_string(),
    };
    assert_eq!(event.service_name(), "test-service");
}

#[test]
fn test_mcp_event_reconciliation_started_service_name_empty() {
    let event = McpEvent::ReconciliationStarted { service_count: 5 };
    assert_eq!(event.service_name(), "");
}

#[test]
fn test_mcp_event_reconciliation_completed_service_name_empty() {
    let event = McpEvent::ReconciliationCompleted {
        started: 5,
        failed: 1,
        duration_ms: 1000,
    };
    assert_eq!(event.service_name(), "");
}

// ============================================================================
// McpEvent event_type Tests
// ============================================================================

#[test]
fn test_mcp_event_service_start_requested_event_type() {
    let event = McpEvent::ServiceStartRequested {
        service_name: "test".to_string(),
    };
    assert_eq!(event.event_type(), "service_start_requested");
}

#[test]
fn test_mcp_event_service_start_completed_event_type() {
    let event = McpEvent::ServiceStartCompleted {
        service_name: "test".to_string(),
        success: true,
        pid: Some(1234),
        port: Some(8080),
        error: None,
        duration_ms: 100,
    };
    assert_eq!(event.event_type(), "service_start_completed");
}

#[test]
fn test_mcp_event_service_started_event_type() {
    let event = McpEvent::ServiceStarted {
        service_name: "test".to_string(),
        process_id: 1234,
        port: 8080,
    };
    assert_eq!(event.event_type(), "service_started");
}

#[test]
fn test_mcp_event_service_failed_event_type() {
    let event = McpEvent::ServiceFailed {
        service_name: "test".to_string(),
        error: "Error".to_string(),
    };
    assert_eq!(event.event_type(), "service_failed");
}

#[test]
fn test_mcp_event_service_stopped_event_type() {
    let event = McpEvent::ServiceStopped {
        service_name: "test".to_string(),
        exit_code: None,
    };
    assert_eq!(event.event_type(), "service_stopped");
}

#[test]
fn test_mcp_event_health_check_failed_event_type() {
    let event = McpEvent::HealthCheckFailed {
        service_name: "test".to_string(),
        reason: "Timeout".to_string(),
    };
    assert_eq!(event.event_type(), "health_check_failed");
}

#[test]
fn test_mcp_event_schema_updated_event_type() {
    let event = McpEvent::SchemaUpdated {
        service_name: "test".to_string(),
        tool_count: 5,
    };
    assert_eq!(event.event_type(), "schema_updated");
}

#[test]
fn test_mcp_event_service_restart_requested_event_type() {
    let event = McpEvent::ServiceRestartRequested {
        service_name: "test".to_string(),
        reason: "Manual".to_string(),
    };
    assert_eq!(event.event_type(), "service_restart_requested");
}

#[test]
fn test_mcp_event_reconciliation_started_event_type() {
    let event = McpEvent::ReconciliationStarted { service_count: 5 };
    assert_eq!(event.event_type(), "reconciliation_started");
}

#[test]
fn test_mcp_event_reconciliation_completed_event_type() {
    let event = McpEvent::ReconciliationCompleted {
        started: 5,
        failed: 1,
        duration_ms: 1000,
    };
    assert_eq!(event.event_type(), "reconciliation_completed");
}

// ============================================================================
// McpEvent Constructors Tests
// ============================================================================

#[test]
fn test_mcp_event_start_completed_success() {
    let event =
        McpEvent::start_completed_success("test-service".to_string(), 1234, 8080, 100);

    match event {
        McpEvent::ServiceStartCompleted {
            service_name,
            success,
            pid,
            port,
            error,
            duration_ms,
        } => {
            assert_eq!(service_name, "test-service");
            assert!(success);
            assert_eq!(pid, Some(1234));
            assert_eq!(port, Some(8080));
            assert!(error.is_none());
            assert_eq!(duration_ms, 100);
        }
        _ => panic!("Expected ServiceStartCompleted"),
    }
}

#[test]
fn test_mcp_event_start_completed_failure() {
    let event = McpEvent::start_completed_failure(
        "test-service".to_string(),
        "Connection failed".to_string(),
        200,
    );

    match event {
        McpEvent::ServiceStartCompleted {
            service_name,
            success,
            pid,
            port,
            error,
            duration_ms,
        } => {
            assert_eq!(service_name, "test-service");
            assert!(!success);
            assert!(pid.is_none());
            assert!(port.is_none());
            assert_eq!(error, Some("Connection failed".to_string()));
            assert_eq!(duration_ms, 200);
        }
        _ => panic!("Expected ServiceStartCompleted"),
    }
}

// ============================================================================
// McpEvent Clone and Debug Tests
// ============================================================================

#[test]
fn test_mcp_event_clone() {
    let event = McpEvent::ServiceStarted {
        service_name: "test-service".to_string(),
        process_id: 1234,
        port: 8080,
    };

    let cloned = event.clone();
    assert_eq!(cloned.service_name(), event.service_name());
}

#[test]
fn test_mcp_event_debug() {
    let event = McpEvent::ServiceStarted {
        service_name: "test-service".to_string(),
        process_id: 1234,
        port: 8080,
    };

    let debug_str = format!("{:?}", event);
    assert!(debug_str.contains("ServiceStarted"));
    assert!(debug_str.contains("test-service"));
}

// ============================================================================
// McpEvent Serialization Tests
// ============================================================================

#[test]
fn test_mcp_event_serialize_service_started() {
    let event = McpEvent::ServiceStarted {
        service_name: "test-service".to_string(),
        process_id: 1234,
        port: 8080,
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("service_started"));
    assert!(json.contains("test-service"));
    assert!(json.contains("1234"));
    assert!(json.contains("8080"));
}

#[test]
fn test_mcp_event_serialize_service_failed() {
    let event = McpEvent::ServiceFailed {
        service_name: "test-service".to_string(),
        error: "Connection refused".to_string(),
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("service_failed"));
    assert!(json.contains("Connection refused"));
}

#[test]
fn test_mcp_event_serialize_reconciliation_started() {
    let event = McpEvent::ReconciliationStarted { service_count: 10 };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("reconciliation_started"));
    assert!(json.contains("10"));
}

#[test]
fn test_mcp_event_serialize_reconciliation_completed() {
    let event = McpEvent::ReconciliationCompleted {
        started: 5,
        failed: 1,
        duration_ms: 1500,
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("reconciliation_completed"));
    assert!(json.contains("1500"));
}

#[test]
fn test_mcp_event_deserialize_service_started() {
    let json = r#"{"type":"service_started","service_name":"test-service","process_id":1234,"port":8080}"#;
    let event: McpEvent = serde_json::from_str(json).unwrap();

    assert_eq!(event.service_name(), "test-service");
    assert_eq!(event.event_type(), "service_started");
}

#[test]
fn test_mcp_event_roundtrip() {
    let events = [
        McpEvent::ServiceStartRequested {
            service_name: "test".to_string(),
        },
        McpEvent::ServiceStarted {
            service_name: "test".to_string(),
            process_id: 1234,
            port: 8080,
        },
        McpEvent::ServiceFailed {
            service_name: "test".to_string(),
            error: "Error".to_string(),
        },
        McpEvent::ServiceStopped {
            service_name: "test".to_string(),
            exit_code: Some(0),
        },
        McpEvent::ReconciliationStarted { service_count: 5 },
        McpEvent::ReconciliationCompleted {
            started: 5,
            failed: 1,
            duration_ms: 1000,
        },
    ];

    for event in events {
        let json = serde_json::to_string(&event).unwrap();
        let parsed: McpEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type(), event.event_type());
    }
}
