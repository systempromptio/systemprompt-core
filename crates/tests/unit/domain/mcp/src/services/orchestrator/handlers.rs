use systemprompt_mcp::services::orchestrator::handlers::{
    EventHandler, HealthCheckHandler, MonitoringHandler,
};
use systemprompt_mcp::services::orchestrator::McpEvent;

#[test]
fn health_check_handler_new_returns_default_state() {
    let handler = HealthCheckHandler::new();
    let debug = format!("{:?}", handler);
    assert!(debug.contains("HealthCheckHandler"));
}

#[test]
fn health_check_handler_default_matches_new() {
    let from_new = HealthCheckHandler::new();
    let from_default = HealthCheckHandler::default();
    let debug_new = format!("{:?}", from_new);
    let debug_default = format!("{:?}", from_default);
    assert!(debug_new.contains("max_failures: 3"));
    assert!(debug_default.contains("max_failures: 3"));
}

#[test]
fn health_check_handler_handles_health_check_failed() {
    let handler = HealthCheckHandler::new();
    let event = McpEvent::HealthCheckFailed {
        service_name: "svc".to_string(),
        reason: "timeout".to_string(),
    };
    assert!(handler.handles(&event));
}

#[test]
fn health_check_handler_handles_service_started() {
    let handler = HealthCheckHandler::new();
    let event = McpEvent::ServiceStarted {
        service_name: "svc".to_string(),
        process_id: 1,
        port: 8080,
    };
    assert!(handler.handles(&event));
}

#[test]
fn health_check_handler_does_not_handle_service_failed() {
    let handler = HealthCheckHandler::new();
    let event = McpEvent::ServiceFailed {
        service_name: "svc".to_string(),
        error: "crash".to_string(),
    };
    assert!(!handler.handles(&event));
}

#[test]
fn health_check_handler_does_not_handle_service_stopped() {
    let handler = HealthCheckHandler::new();
    let event = McpEvent::ServiceStopped {
        service_name: "svc".to_string(),
        exit_code: Some(0),
    };
    assert!(!handler.handles(&event));
}

#[test]
fn health_check_handler_does_not_handle_schema_updated() {
    let handler = HealthCheckHandler::new();
    let event = McpEvent::SchemaUpdated {
        service_name: "svc".to_string(),
        tool_count: 5,
    };
    assert!(!handler.handles(&event));
}

#[test]
fn health_check_handler_does_not_handle_reconciliation_started() {
    let handler = HealthCheckHandler::new();
    let event = McpEvent::ReconciliationStarted { service_count: 3 };
    assert!(!handler.handles(&event));
}

#[test]
fn health_check_handler_does_not_handle_reconciliation_completed() {
    let handler = HealthCheckHandler::new();
    let event = McpEvent::ReconciliationCompleted {
        started: 3,
        failed: 0,
        duration_ms: 100,
    };
    assert!(!handler.handles(&event));
}

#[test]
fn health_check_handler_does_not_handle_service_start_requested() {
    let handler = HealthCheckHandler::new();
    let event = McpEvent::ServiceStartRequested {
        service_name: "svc".to_string(),
    };
    assert!(!handler.handles(&event));
}

#[test]
fn health_check_handler_does_not_handle_service_restart_requested() {
    let handler = HealthCheckHandler::new();
    let event = McpEvent::ServiceRestartRequested {
        service_name: "svc".to_string(),
        reason: "manual".to_string(),
    };
    assert!(!handler.handles(&event));
}

#[test]
fn health_check_handler_does_not_handle_service_start_completed() {
    let handler = HealthCheckHandler::new();
    let event = McpEvent::ServiceStartCompleted {
        service_name: "svc".to_string(),
        success: true,
        pid: Some(100),
        port: Some(8080),
        error: None,
        duration_ms: 50,
    };
    assert!(!handler.handles(&event));
}

#[test]
fn health_check_handler_name_returns_expected() {
    let handler = HealthCheckHandler::new();
    assert_eq!(handler.name(), "health_check");
}

#[test]
fn health_check_handler_with_restart_sender_debug() {
    let (sender, _) = tokio::sync::broadcast::channel::<McpEvent>(10);
    let handler = HealthCheckHandler::new().with_restart_sender(sender);
    let debug = format!("{:?}", handler);
    assert!(debug.contains("HealthCheckHandler"));
    assert!(debug.contains("restart_sender: Some"));
}

#[test]
fn health_check_handler_without_restart_sender_debug() {
    let handler = HealthCheckHandler::new();
    let debug = format!("{:?}", handler);
    assert!(debug.contains("restart_sender: None"));
}

#[test]
fn monitoring_handler_name_returns_monitoring() {
    let handler = MonitoringHandler;
    assert_eq!(handler.name(), "monitoring");
}

#[test]
fn monitoring_handler_handles_all_events_by_default() {
    let handler = MonitoringHandler;
    let event = McpEvent::ServiceStarted {
        service_name: "svc".to_string(),
        process_id: 1,
        port: 80,
    };
    assert!(handler.handles(&event));
}

#[test]
fn monitoring_handler_handles_reconciliation_events() {
    let handler = MonitoringHandler;
    let event = McpEvent::ReconciliationStarted { service_count: 5 };
    assert!(handler.handles(&event));
}

#[test]
fn monitoring_handler_debug() {
    let handler = MonitoringHandler;
    let debug = format!("{:?}", handler);
    assert!(debug.contains("MonitoringHandler"));
}

#[test]
fn monitoring_handler_copy() {
    let handler = MonitoringHandler;
    let copied = handler;
    assert_eq!(copied.name(), "monitoring");
}

#[test]
fn monitoring_handler_clone() {
    let handler = MonitoringHandler;
    let cloned = handler.clone();
    assert_eq!(cloned.name(), "monitoring");
}
