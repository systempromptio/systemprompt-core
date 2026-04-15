use systemprompt_agent::AgentEvent;
use systemprompt_identifiers::AgentId;

#[test]
fn test_agent_event_start_requested() {
    let event = AgentEvent::AgentStartRequested {
        agent_id: AgentId::new("agent-123"),
    };

    assert_eq!(event.agent_id().map(|a| a.as_str()), Some("agent-123"));
    assert_eq!(event.event_type(), "agent_start_requested");
}

#[test]
fn test_agent_event_start_completed_success() {
    let event = AgentEvent::AgentStartCompleted {
        agent_id: AgentId::new("agent-456"),
        success: true,
        pid: Some(12345),
        port: Some(8080),
        error: None,
    };

    assert_eq!(event.agent_id().map(|a| a.as_str()), Some("agent-456"));
    assert_eq!(event.event_type(), "agent_start_completed");
}

#[test]
fn test_agent_event_start_completed_failure() {
    let event = AgentEvent::AgentStartCompleted {
        agent_id: AgentId::new("agent-789"),
        success: false,
        pid: None,
        port: None,
        error: Some("Failed to bind to port".to_string()),
    };

    assert_eq!(event.agent_id().map(|a| a.as_str()), Some("agent-789"));
    assert_eq!(event.event_type(), "agent_start_completed");
}

#[test]
fn test_agent_event_started() {
    let event = AgentEvent::AgentStarted {
        agent_id: AgentId::new("agent-started"),
        pid: 54321,
        port: 9000,
    };

    assert_eq!(event.agent_id().map(|a| a.as_str()), Some("agent-started"));
    assert_eq!(event.event_type(), "agent_started");
}

#[test]
fn test_agent_event_failed() {
    let event = AgentEvent::AgentFailed {
        agent_id: AgentId::new("agent-failed"),
        error: "Process crashed".to_string(),
    };

    assert_eq!(event.agent_id().map(|a| a.as_str()), Some("agent-failed"));
    assert_eq!(event.event_type(), "agent_failed");
}

#[test]
fn test_agent_event_stopped() {
    let event = AgentEvent::AgentStopped {
        agent_id: AgentId::new("agent-stopped"),
        exit_code: Some(0),
    };

    assert_eq!(event.agent_id().map(|a| a.as_str()), Some("agent-stopped"));
    assert_eq!(event.event_type(), "agent_stopped");
}

#[test]
fn test_agent_event_stopped_no_exit_code() {
    let event = AgentEvent::AgentStopped {
        agent_id: AgentId::new("agent-killed"),
        exit_code: None,
    };

    assert_eq!(event.agent_id().map(|a| a.as_str()), Some("agent-killed"));
    assert_eq!(event.event_type(), "agent_stopped");
}

#[test]
fn test_agent_event_disabled() {
    let event = AgentEvent::AgentDisabled {
        agent_id: AgentId::new("agent-disabled"),
    };

    assert_eq!(event.agent_id().map(|a| a.as_str()), Some("agent-disabled"));
    assert_eq!(event.event_type(), "agent_disabled");
}

#[test]
fn test_agent_event_health_check_failed() {
    let event = AgentEvent::HealthCheckFailed {
        agent_id: AgentId::new("agent-unhealthy"),
        reason: "Connection timeout".to_string(),
    };

    assert_eq!(event.agent_id().map(|a| a.as_str()), Some("agent-unhealthy"));
    assert_eq!(event.event_type(), "health_check_failed");
}

#[test]
fn test_agent_event_restart_requested() {
    let event = AgentEvent::AgentRestartRequested {
        agent_id: AgentId::new("agent-restart"),
        reason: "Configuration changed".to_string(),
    };

    assert_eq!(event.agent_id().map(|a| a.as_str()), Some("agent-restart"));
    assert_eq!(event.event_type(), "agent_restart_requested");
}

#[test]
fn test_agent_event_reconciliation_started() {
    let event = AgentEvent::ReconciliationStarted { agent_count: 5 };

    assert!(event.agent_id().is_none());
    assert_eq!(event.event_type(), "reconciliation_started");
}

#[test]
fn test_agent_event_reconciliation_completed() {
    let event = AgentEvent::ReconciliationCompleted {
        started: 5,
        failed: 1,
    };

    assert!(event.agent_id().is_none());
    assert_eq!(event.event_type(), "reconciliation_completed");
}

#[test]
fn test_agent_event_serialize_start_requested() {
    let event = AgentEvent::AgentStartRequested {
        agent_id: AgentId::new("ser-1"),
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("agent_start_requested"));
    assert!(json.contains("ser-1"));
}

#[test]
fn test_agent_event_serialize_started() {
    let event = AgentEvent::AgentStarted {
        agent_id: AgentId::new("ser-2"),
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
        agent_id: AgentId::new("ser-3"),
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

#[test]
fn test_agent_event_debug() {
    let event = AgentEvent::AgentStartRequested {
        agent_id: AgentId::new("debug-test"),
    };

    let debug_str = format!("{:?}", event);
    assert!(debug_str.contains("AgentStartRequested"));
    assert!(debug_str.contains("debug-test"));
}

#[test]
fn test_all_event_types_are_unique() {
    let events = vec![
        AgentEvent::AgentStartRequested {
            agent_id: AgentId::new(""),
        },
        AgentEvent::AgentStartCompleted {
            agent_id: AgentId::new(""),
            success: true,
            pid: None,
            port: None,
            error: None,
        },
        AgentEvent::AgentStarted {
            agent_id: AgentId::new(""),
            pid: 0,
            port: 0,
        },
        AgentEvent::AgentFailed {
            agent_id: AgentId::new(""),
            error: "".to_string(),
        },
        AgentEvent::AgentStopped {
            agent_id: AgentId::new(""),
            exit_code: None,
        },
        AgentEvent::AgentDisabled {
            agent_id: AgentId::new(""),
        },
        AgentEvent::HealthCheckFailed {
            agent_id: AgentId::new(""),
            reason: "".to_string(),
        },
        AgentEvent::AgentRestartRequested {
            agent_id: AgentId::new(""),
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
