use systemprompt_agent::AgentEvent;
use systemprompt_identifiers::AgentId;

#[test]
fn test_agent_event_roundtrip_start_requested() {
    let event = AgentEvent::AgentStartRequested {
        agent_id: AgentId::new("roundtrip-1"),
    };

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(
        deserialized.agent_id().map(|a| a.as_str()),
        Some("roundtrip-1")
    );
    assert_eq!(deserialized.event_type(), "agent_start_requested");
}

#[test]
fn test_agent_event_roundtrip_start_completed_success() {
    let event = AgentEvent::AgentStartCompleted {
        agent_id: AgentId::new("roundtrip-2"),
        success: true,
        pid: Some(9999),
        port: Some(3000),
        error: None,
    };

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(
        deserialized.agent_id().map(|a| a.as_str()),
        Some("roundtrip-2")
    );
    assert_eq!(deserialized.event_type(), "agent_start_completed");
}

#[test]
fn test_agent_event_roundtrip_start_completed_failure() {
    let event = AgentEvent::AgentStartCompleted {
        agent_id: AgentId::new("roundtrip-3"),
        success: false,
        pid: None,
        port: None,
        error: Some("port conflict".to_string()),
    };

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(
        deserialized.agent_id().map(|a| a.as_str()),
        Some("roundtrip-3")
    );
    assert_eq!(deserialized.event_type(), "agent_start_completed");
}

#[test]
fn test_agent_event_roundtrip_started() {
    let event = AgentEvent::AgentStarted {
        agent_id: AgentId::new("roundtrip-4"),
        pid: 42,
        port: 8080,
    };

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(
        deserialized.agent_id().map(|a| a.as_str()),
        Some("roundtrip-4")
    );
    assert_eq!(deserialized.event_type(), "agent_started");
}

#[test]
fn test_agent_event_roundtrip_failed() {
    let event = AgentEvent::AgentFailed {
        agent_id: AgentId::new("roundtrip-5"),
        error: "segfault".to_string(),
    };

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(
        deserialized.agent_id().map(|a| a.as_str()),
        Some("roundtrip-5")
    );
    assert_eq!(deserialized.event_type(), "agent_failed");
}

#[test]
fn test_agent_event_roundtrip_stopped_with_code() {
    let event = AgentEvent::AgentStopped {
        agent_id: AgentId::new("roundtrip-6"),
        exit_code: Some(137),
    };

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(
        deserialized.agent_id().map(|a| a.as_str()),
        Some("roundtrip-6")
    );
    assert_eq!(deserialized.event_type(), "agent_stopped");
}

#[test]
fn test_agent_event_roundtrip_stopped_no_code() {
    let event = AgentEvent::AgentStopped {
        agent_id: AgentId::new("roundtrip-7"),
        exit_code: None,
    };

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(
        deserialized.agent_id().map(|a| a.as_str()),
        Some("roundtrip-7")
    );
}

#[test]
fn test_agent_event_roundtrip_disabled() {
    let event = AgentEvent::AgentDisabled {
        agent_id: AgentId::new("roundtrip-8"),
    };

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(
        deserialized.agent_id().map(|a| a.as_str()),
        Some("roundtrip-8")
    );
    assert_eq!(deserialized.event_type(), "agent_disabled");
}

#[test]
fn test_agent_event_roundtrip_health_check_failed() {
    let event = AgentEvent::HealthCheckFailed {
        agent_id: AgentId::new("roundtrip-9"),
        reason: "timeout after 15s".to_string(),
    };

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(
        deserialized.agent_id().map(|a| a.as_str()),
        Some("roundtrip-9")
    );
    assert_eq!(deserialized.event_type(), "health_check_failed");
}

#[test]
fn test_agent_event_roundtrip_restart_requested() {
    let event = AgentEvent::AgentRestartRequested {
        agent_id: AgentId::new("roundtrip-10"),
        reason: "config change".to_string(),
    };

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(
        deserialized.agent_id().map(|a| a.as_str()),
        Some("roundtrip-10")
    );
    assert_eq!(deserialized.event_type(), "agent_restart_requested");
}

#[test]
fn test_agent_event_roundtrip_reconciliation_started() {
    let event = AgentEvent::ReconciliationStarted { agent_count: 42 };

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();

    assert!(deserialized.agent_id().is_none());
    assert_eq!(deserialized.event_type(), "reconciliation_started");
}

#[test]
fn test_agent_event_roundtrip_reconciliation_completed() {
    let event = AgentEvent::ReconciliationCompleted {
        started: 10,
        failed: 3,
    };

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AgentEvent = serde_json::from_str(&json).unwrap();

    assert!(deserialized.agent_id().is_none());
    assert_eq!(deserialized.event_type(), "reconciliation_completed");
}

#[test]
fn test_agent_event_clone_preserves_data() {
    let event = AgentEvent::AgentStarted {
        agent_id: AgentId::new("clone-test"),
        pid: 777,
        port: 9090,
    };

    let cloned = event.clone();
    assert_eq!(cloned.agent_id().map(|a| a.as_str()), Some("clone-test"));
    assert_eq!(cloned.event_type(), "agent_started");
}

#[test]
fn test_agent_event_clone_start_completed() {
    let event = AgentEvent::AgentStartCompleted {
        agent_id: AgentId::new("clone-2"),
        success: false,
        pid: None,
        port: None,
        error: Some("test error".to_string()),
    };

    let cloned = event.clone();
    assert_eq!(cloned.agent_id().map(|a| a.as_str()), Some("clone-2"));
}

#[test]
fn test_agent_event_deserialize_from_json_string() {
    let json = r#"{"type":"agent_started","agent_id":"from-json","pid":1234,"port":8080}"#;
    let event: AgentEvent = serde_json::from_str(json).unwrap();

    assert_eq!(event.agent_id().map(|a| a.as_str()), Some("from-json"));
    assert_eq!(event.event_type(), "agent_started");
}

#[test]
fn test_agent_event_deserialize_agent_failed_from_json() {
    let json = r#"{"type":"agent_failed","agent_id":"fail-deser","error":"boom"}"#;
    let event: AgentEvent = serde_json::from_str(json).unwrap();

    assert_eq!(event.agent_id().map(|a| a.as_str()), Some("fail-deser"));
    assert_eq!(event.event_type(), "agent_failed");
}

#[test]
fn test_agent_event_deserialize_reconciliation_started_from_json() {
    let json = r#"{"type":"reconciliation_started","agent_count":7}"#;
    let event: AgentEvent = serde_json::from_str(json).unwrap();

    assert_eq!(event.event_type(), "reconciliation_started");
}

#[test]
fn test_agent_event_serialize_contains_type_tag() {
    let event = AgentEvent::AgentDisabled {
        agent_id: AgentId::new("tag-test"),
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""type":"agent_disabled""#));
}

#[test]
fn test_agent_event_serialize_health_check_failed_fields() {
    let event = AgentEvent::HealthCheckFailed {
        agent_id: AgentId::new("hc-test"),
        reason: "connection refused".to_string(),
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("hc-test"));
    assert!(json.contains("connection refused"));
    assert!(json.contains("health_check_failed"));
}

#[test]
fn test_agent_event_serialize_restart_requested_fields() {
    let event = AgentEvent::AgentRestartRequested {
        agent_id: AgentId::new("restart-ser"),
        reason: "manual restart".to_string(),
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("restart-ser"));
    assert!(json.contains("manual restart"));
}
