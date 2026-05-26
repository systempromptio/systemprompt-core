//! Unit tests for the `MonitoringService` facade.

use std::collections::HashMap;
use systemprompt_mcp::services::monitoring::MonitoringService;

#[test]
fn test_new_default() {
    let _s: MonitoringService = MonitoringService::new();
    let _t: MonitoringService = MonitoringService::default();
}

#[test]
fn test_display_status_empty_inputs_does_not_panic() {
    // Both inputs are empty; the function should be a no-op (it prints).
    MonitoringService::display_status(&[], &HashMap::new());
}

#[test]
fn test_clone_copy() {
    let a = MonitoringService::new();
    let b = a;
    let _c = b;
}

#[test]
fn test_debug() {
    let s = MonitoringService::new();
    let d = format!("{:?}", s);
    assert!(d.contains("MonitoringService"));
}
