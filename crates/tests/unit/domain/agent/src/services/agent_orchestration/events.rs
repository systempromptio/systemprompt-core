//! Unit tests for agent orchestration events
//!
//! Tests cover:
//! - AgentEvent variants
//! - Event serialization and deserialization
//! - Helper methods (agent_id, event_type)

use systemprompt_agent::AgentEvent;

// ============================================================================
// AgentEvent Variant Tests
// ============================================================================

#[test]
fn test_agent_event_start_requested() {
    let event = AgentEvent::AgentStartRequested {
        agent_id: "agent-123".to_string(),
    };

    assert_eq!(event.agent_id(), "agent-123");
    assert_eq!(event.event_type(), "agent_start_requested");
}

#[test]
fn test_agent_event_start_completed_success() {
    let event = AgentEvent::AgentStartCompleted {
        agent_id: "agent-456".to_string(),
        success: true,
        pid: Some(12345),
        port: Some(8080),
        error: None,
    };

    assert_eq!(event.agent_id(), "agent-456");
    assert_eq!(event.event_type(), "agent_start_completed");
}

#[test]
fn test_agent_event_start_completed_failure() {
    let event = AgentEvent::AgentStartCompleted {
        agent_id: "agent-789".to_string(),
        success: false,
        pid: None,
        port: None,
        error: Some("Failed to bind to port".to_string()),
    };

    assert_eq!(event.agent_id(), "agent-789");
    assert_eq!(event.event_type(), "agent_start_completed");
}

#[test]
fn test_agent_event_started() {
    let event = AgentEvent::AgentStarted {
        agent_id: "agent-started".to_string(),
        pid: 54321,
        port: 9000,
    };

    assert_eq!(event.agent_id(), "agent-started");
    assert_eq!(event.event_type(), "agent_started");
}

#[test]
fn test_agent_event_failed() {
    let event = AgentEvent::AgentFailed {
        agent_id: "agent-failed".to_string(),
        error: "Process crashed".to_string(),
    };

    assert_eq!(event.agent_id(), "agent-failed");
    assert_eq!(event.event_type(), "agent_failed");
}

#[test]
fn test_agent_event_stopped() {
    let event = AgentEvent::AgentStopped {
        agent_id: "agent-stopped".to_string(),
        exit_code: Some(0),
    };

    assert_eq!(event.agent_id(), "agent-stopped");
    assert_eq!(event.event_type(), "agent_stopped");
}

#[test]
fn test_agent_event_stopped_no_exit_code() {
    let event = AgentEvent::AgentStopped {
        agent_id: "agent-killed".to_string(),
        exit_code: None,
    };

    assert_eq!(event.agent_id(), "agent-killed");
    assert_eq!(event.event_type(), "agent_stopped");
}

#[test]
fn test_agent_event_disabled() {
    let event = AgentEvent::AgentDisabled {
        agent_id: "agent-disabled".to_string(),
    };

    assert_eq!(event.agent_id(), "agent-disabled");
    assert_eq!(event.event_type(), "agent_disabled");
}

#[test]
fn test_agent_event_health_check_failed() {
    let event = AgentEvent::HealthCheckFailed {
        agent_id: "agent-unhealthy".to_string(),
        reason: "Connection timeout".to_string(),
    };

    assert_eq!(event.agent_id(), "agent-unhealthy");
    assert_eq!(event.event_type(), "health_check_failed");
}

#[test]
fn test_agent_event_restart_requested() {
    let event = AgentEvent::AgentRestartRequested {
        agent_id: "agent-restart".to_string(),
        reason: "Configuration changed".to_string(),
    };

    assert_eq!(event.agent_id(), "agent-restart");
    assert_eq!(event.event_type(), "agent_restart_requested");
}

#[test]
fn test_agent_event_reconciliation_started() {
    let event = AgentEvent::ReconciliationStarted { agent_count: 5 };

    assert_eq!(event.agent_id(), "");
    assert_eq!(event.event_type(), "reconciliation_started");
}

#[test]
fn test_agent_event_reconciliation_completed() {
    let event = AgentEvent::ReconciliationCompleted {
        started: 5,
        failed: 1,
    };

    assert_eq!(event.agent_id(), "");
    assert_eq!(event.event_type(), "reconciliation_completed");
}

// ============================================================================
// Serialization Tests
// ============================================================================

#[test]
fn test_agent_event_serialize_start_requested() {
    let event = AgentEvent::AgentStartRequested {
        agent_id: "ser-1".to_string(),
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("agent_start_requested"));
    assert!(json.contains("ser-1"));
}

#[test]
fn test_agent_event_serialize_started() {
    let event = AgentEvent::AgentStarted {
        agent_id: "ser-2".to_string(),
        pid: 1234,
        port: 8080,
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("agent_started"));
    assert!(json.contains("1234"));
    assert!(json.contains("8080"));
}

#[test]
fn test_agent_event_serialize_failed() {
    let event = AgentEvent::AgentFailed {
        agent_id: "ser-3".to_string(),
        error: "Test error".to_string(),
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("agent_failed"));
    assert!(json.contains("Test error"));
}

#[test]
fn test_agent_event_serialize_reconciliation() {
    let event = AgentEvent::ReconciliationCompleted {
        started: 10,
        failed: 2,
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("reconciliation_completed"));
    assert!(json.contains("10"));
    assert!(json.contains("2"));
}

// ============================================================================
// Deserialization Tests
// ============================================================================

#[test]
fn test_agent_event_deserialize_start_requested() {
    let json = r#"{"type": "agent_start_requested", "agent_id": "deser-1"}"#;
    let event: AgentEvent = serde_json::from_str(json).unwrap();

    assert_eq!(event.agent_id(), "deser-1");
    assert_eq!(event.event_type(), "agent_start_requested");
}

#[test]
fn test_agent_event_deserialize_started() {
    let json = r#"{"type": "agent_started", "agent_id": "deser-2", "pid": 5678, "port": 3000}"#;
    let event: AgentEvent = serde_json::from_str(json).unwrap();

    match event {
        AgentEvent::AgentStarted { pid, port, .. } => {
            assert_eq!(pid, 5678);
            assert_eq!(port, 3000);
        }
        _ => panic!("Expected AgentStarted variant"),
    }
}

#[test]
fn test_agent_event_deserialize_stopped() {
    let json = r#"{"type": "agent_stopped", "agent_id": "deser-3", "exit_code": 1}"#;
    let event: AgentEvent = serde_json::from_str(json).unwrap();

    match event {
        AgentEvent::AgentStopped { exit_code, .. } => {
            assert_eq!(exit_code, Some(1));
        }
        _ => panic!("Expected AgentStopped variant"),
    }
}

#[test]
fn test_agent_event_deserialize_health_check_failed() {
    let json =
        r#"{"type": "health_check_failed", "agent_id": "deser-4", "reason": "Port unreachable"}"#;
    let event: AgentEvent = serde_json::from_str(json).unwrap();

    match event {
        AgentEvent::HealthCheckFailed { reason, .. } => {
            assert_eq!(reason, "Port unreachable");
        }
        _ => panic!("Expected HealthCheckFailed variant"),
    }
}

// ============================================================================
// Debug and Clone Tests
// ============================================================================

#[test]
fn test_agent_event_debug() {
    let event = AgentEvent::AgentStartRequested {
        agent_id: "debug-test".to_string(),
    };

    let debug_str = format!("{:?}", event);
    assert!(debug_str.contains("AgentStartRequested"));
    assert!(debug_str.contains("debug-test"));
}

#[test]
fn test_agent_event_clone() {
    let event = AgentEvent::AgentStarted {
        agent_id: "clone-test".to_string(),
        pid: 9999,
        port: 7777,
    };

    let cloned = event.clone();
    assert_eq!(cloned.agent_id(), "clone-test");
}

// ============================================================================
// Event Type Consistency Tests
// ============================================================================

#[test]
fn test_all_event_types_are_unique() {
    let events = vec![
        AgentEvent::AgentStartRequested {
            agent_id: "".to_string(),
        },
        AgentEvent::AgentStartCompleted {
            agent_id: "".to_string(),
            success: true,
            pid: None,
            port: None,
            error: None,
        },
        AgentEvent::AgentStarted {
            agent_id: "".to_string(),
            pid: 0,
            port: 0,
        },
        AgentEvent::AgentFailed {
            agent_id: "".to_string(),
            error: "".to_string(),
        },
        AgentEvent::AgentStopped {
            agent_id: "".to_string(),
            exit_code: None,
        },
        AgentEvent::AgentDisabled {
            agent_id: "".to_string(),
        },
        AgentEvent::HealthCheckFailed {
            agent_id: "".to_string(),
            reason: "".to_string(),
        },
        AgentEvent::AgentRestartRequested {
            agent_id: "".to_string(),
            reason: "".to_string(),
        },
        AgentEvent::ReconciliationStarted { agent_count: 0 },
        AgentEvent::ReconciliationCompleted {
            started: 0,
            failed: 0,
        },
    ];

    let event_types: Vec<&str> = events.iter().map(|e| e.event_type()).collect();

    for (i, event_type) in event_types.iter().enumerate() {
        for (j, other_type) in event_types.iter().enumerate() {
            if i != j {
                assert_ne!(
                    event_type, other_type,
                    "Duplicate event type: {}",
                    event_type
                );
            }
        }
    }
}
