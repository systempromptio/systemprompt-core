use systemprompt_mcp::services::orchestrator::McpEvent;

#[test]
fn mcp_event_deserialize_service_start_requested() {
    let json = r#"{"type":"service_start_requested","service_name":"my-svc"}"#;
    let event: McpEvent = serde_json::from_str(json).unwrap();
    assert_eq!(event.service_name(), "my-svc");
    assert_eq!(event.event_type(), "service_start_requested");
}

#[test]
fn mcp_event_deserialize_service_started() {
    let json = r#"{"type":"service_started","service_name":"svc","process_id":42,"port":9090}"#;
    let event: McpEvent = serde_json::from_str(json).unwrap();
    assert_eq!(event.service_name(), "svc");
    assert_eq!(event.event_type(), "service_started");
}

#[test]
fn mcp_event_deserialize_service_failed() {
    let json = r#"{"type":"service_failed","service_name":"svc","error":"boom"}"#;
    let event: McpEvent = serde_json::from_str(json).unwrap();
    assert_eq!(event.service_name(), "svc");
    match &event {
        McpEvent::ServiceFailed { error, .. } => assert_eq!(error, "boom"),
        _ => panic!("Expected ServiceFailed"),
    }
}

#[test]
fn mcp_event_deserialize_service_stopped_with_exit_code() {
    let json = r#"{"type":"service_stopped","service_name":"svc","exit_code":1}"#;
    let event: McpEvent = serde_json::from_str(json).unwrap();
    match &event {
        McpEvent::ServiceStopped { exit_code, .. } => assert_eq!(*exit_code, Some(1)),
        _ => panic!("Expected ServiceStopped"),
    }
}

#[test]
fn mcp_event_deserialize_service_stopped_null_exit_code() {
    let json = r#"{"type":"service_stopped","service_name":"svc","exit_code":null}"#;
    let event: McpEvent = serde_json::from_str(json).unwrap();
    match &event {
        McpEvent::ServiceStopped { exit_code, .. } => assert!(exit_code.is_none()),
        _ => panic!("Expected ServiceStopped"),
    }
}

#[test]
fn mcp_event_deserialize_health_check_failed() {
    let json = r#"{"type":"health_check_failed","service_name":"svc","reason":"timeout"}"#;
    let event: McpEvent = serde_json::from_str(json).unwrap();
    assert_eq!(event.event_type(), "health_check_failed");
    match &event {
        McpEvent::HealthCheckFailed { reason, .. } => assert_eq!(reason, "timeout"),
        _ => panic!("Expected HealthCheckFailed"),
    }
}

#[test]
fn mcp_event_deserialize_schema_updated() {
    let json = r#"{"type":"schema_updated","service_name":"svc","tool_count":7}"#;
    let event: McpEvent = serde_json::from_str(json).unwrap();
    match &event {
        McpEvent::SchemaUpdated { tool_count, .. } => assert_eq!(*tool_count, 7),
        _ => panic!("Expected SchemaUpdated"),
    }
}

#[test]
fn mcp_event_deserialize_reconciliation_started() {
    let json = r#"{"type":"reconciliation_started","service_count":3}"#;
    let event: McpEvent = serde_json::from_str(json).unwrap();
    assert_eq!(event.service_name(), "");
    match &event {
        McpEvent::ReconciliationStarted { service_count } => assert_eq!(*service_count, 3),
        _ => panic!("Expected ReconciliationStarted"),
    }
}

#[test]
fn mcp_event_deserialize_reconciliation_completed() {
    let json = r#"{"type":"reconciliation_completed","started":5,"failed":2,"duration_ms":500}"#;
    let event: McpEvent = serde_json::from_str(json).unwrap();
    match &event {
        McpEvent::ReconciliationCompleted {
            started,
            failed,
            duration_ms,
        } => {
            assert_eq!(*started, 5);
            assert_eq!(*failed, 2);
            assert_eq!(*duration_ms, 500);
        },
        _ => panic!("Expected ReconciliationCompleted"),
    }
}

#[test]
fn mcp_event_deserialize_service_start_completed_success() {
    let json = r#"{"type":"service_start_completed","service_name":"svc","success":true,"pid":100,"port":8080,"error":null,"duration_ms":250}"#;
    let event: McpEvent = serde_json::from_str(json).unwrap();
    match &event {
        McpEvent::ServiceStartCompleted {
            success,
            pid,
            port,
            error,
            duration_ms,
            ..
        } => {
            assert!(*success);
            assert_eq!(*pid, Some(100));
            assert_eq!(*port, Some(8080));
            assert!(error.is_none());
            assert_eq!(*duration_ms, 250);
        },
        _ => panic!("Expected ServiceStartCompleted"),
    }
}

#[test]
fn mcp_event_deserialize_service_start_completed_failure() {
    let json = r#"{"type":"service_start_completed","service_name":"svc","success":false,"pid":null,"port":null,"error":"crash","duration_ms":50}"#;
    let event: McpEvent = serde_json::from_str(json).unwrap();
    match &event {
        McpEvent::ServiceStartCompleted {
            success,
            pid,
            port,
            error,
            ..
        } => {
            assert!(!*success);
            assert!(pid.is_none());
            assert!(port.is_none());
            assert_eq!(error.as_deref(), Some("crash"));
        },
        _ => panic!("Expected ServiceStartCompleted"),
    }
}

#[test]
fn mcp_event_roundtrip_service_restart_requested() {
    let event = McpEvent::ServiceRestartRequested {
        service_name: "roundtrip-svc".to_string(),
        reason: "config change".to_string(),
    };
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: McpEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.service_name(), "roundtrip-svc");
    assert_eq!(deserialized.event_type(), "service_restart_requested");
}

#[test]
fn mcp_event_roundtrip_all_variants() {
    let events = vec![
        McpEvent::ServiceStartRequested {
            service_name: "s".to_string(),
        },
        McpEvent::start_completed_success("s".to_string(), 1, 80, 10),
        McpEvent::start_completed_failure("s".to_string(), "e".to_string(), 10),
        McpEvent::ServiceStarted {
            service_name: "s".to_string(),
            process_id: 1,
            port: 80,
        },
        McpEvent::ServiceFailed {
            service_name: "s".to_string(),
            error: "e".to_string(),
        },
        McpEvent::ServiceStopped {
            service_name: "s".to_string(),
            exit_code: Some(0),
        },
        McpEvent::HealthCheckFailed {
            service_name: "s".to_string(),
            reason: "r".to_string(),
        },
        McpEvent::SchemaUpdated {
            service_name: "s".to_string(),
            tool_count: 1,
        },
        McpEvent::ServiceRestartRequested {
            service_name: "s".to_string(),
            reason: "r".to_string(),
        },
        McpEvent::ReconciliationStarted { service_count: 1 },
        McpEvent::ReconciliationCompleted {
            started: 1,
            failed: 0,
            duration_ms: 1,
        },
    ];

    for event in &events {
        let json = serde_json::to_string(event).unwrap();
        let roundtripped: McpEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtripped.event_type(), event.event_type());
        assert_eq!(roundtripped.service_name(), event.service_name());
    }
}

#[test]
fn mcp_event_service_name_empty_string_input() {
    let event = McpEvent::ServiceStartRequested {
        service_name: String::new(),
    };
    assert_eq!(event.service_name(), "");
}

#[test]
fn mcp_event_service_name_with_special_characters() {
    let event = McpEvent::ServiceStartRequested {
        service_name: "my-service_v2.0".to_string(),
    };
    assert_eq!(event.service_name(), "my-service_v2.0");
}

#[test]
fn mcp_event_start_completed_success_zero_duration() {
    let event = McpEvent::start_completed_success("svc".to_string(), 1, 80, 0);
    match &event {
        McpEvent::ServiceStartCompleted { duration_ms, .. } => assert_eq!(*duration_ms, 0),
        _ => panic!("Expected ServiceStartCompleted"),
    }
}

#[test]
fn mcp_event_start_completed_failure_empty_error() {
    let event = McpEvent::start_completed_failure("svc".to_string(), String::new(), 100);
    match &event {
        McpEvent::ServiceStartCompleted { error, .. } => {
            assert_eq!(error.as_deref(), Some(""));
        },
        _ => panic!("Expected ServiceStartCompleted"),
    }
}

#[test]
fn mcp_event_clone_preserves_all_fields() {
    let event = McpEvent::ServiceStartCompleted {
        service_name: "clone-test".to_string(),
        success: true,
        pid: Some(999),
        port: Some(3000),
        error: None,
        duration_ms: 42,
    };
    let cloned = event.clone();
    assert_eq!(cloned.service_name(), "clone-test");
    assert_eq!(cloned.event_type(), "service_start_completed");
    match &cloned {
        McpEvent::ServiceStartCompleted {
            pid,
            port,
            duration_ms,
            ..
        } => {
            assert_eq!(*pid, Some(999));
            assert_eq!(*port, Some(3000));
            assert_eq!(*duration_ms, 42);
        },
        _ => panic!("Expected ServiceStartCompleted"),
    }
}

#[test]
fn mcp_event_deserialize_invalid_type_fails() {
    let json = r#"{"type":"nonexistent_event","service_name":"svc"}"#;
    let result = serde_json::from_str::<McpEvent>(json);
    assert!(result.is_err());
}
