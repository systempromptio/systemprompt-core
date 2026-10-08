use systemprompt_identifiers::ServiceName;
use systemprompt_mcp::services::orchestrator::McpEvent;
use systemprompt_mcp::services::orchestrator::subscribers::{
    EventSubscriber, MonitoringSubscriber,
};

#[test]
fn monitoring_handler_name_returns_monitoring() {
    let handler = MonitoringSubscriber;
    assert_eq!(handler.name(), "monitoring");
}

#[test]
fn monitoring_handler_handles_all_events_by_default() {
    let handler = MonitoringSubscriber;
    let event = McpEvent::ServiceStarted {
        service_name: ServiceName::new("svc"),
        process_id: Some(1),
        port: 80,
    };
    assert!(handler.handles(&event));
}

#[test]
fn monitoring_handler_handles_reconciliation_events() {
    let handler = MonitoringSubscriber;
    let event = McpEvent::ReconciliationStarted { service_count: 5 };
    assert!(handler.handles(&event));
}
