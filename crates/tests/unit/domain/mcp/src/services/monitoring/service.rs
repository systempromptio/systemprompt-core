//! Unit tests for the `MonitoringService` facade.

use systemprompt_mcp::services::monitoring::MonitoringService;

#[test]
fn test_new_default() {
    let _s: MonitoringService = MonitoringService::new();
    let _t: MonitoringService = MonitoringService::default();
}

#[test]
fn test_display_status_empty_inputs_does_not_panic() {
    MonitoringService::display_status(&[]);
}

#[test]
fn test_clone_copy() {
    let a = MonitoringService::new();
    let b = a;
    let _c = b;
}
