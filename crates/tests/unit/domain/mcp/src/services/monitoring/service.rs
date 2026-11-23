//! Unit tests for the `MonitoringService` facade.

use systemprompt_mcp::services::monitoring::MonitoringService;

#[test]
fn test_new_default() {
    let _s: MonitoringService =
        MonitoringService::new(systemprompt_test_fixtures::fixture_health_probe());
}

#[test]
fn test_display_status_empty_inputs_does_not_panic() {
    MonitoringService::display_status(&[]);
}

#[test]
fn test_clone_keeps_the_probe() {
    let a = MonitoringService::new(systemprompt_test_fixtures::fixture_health_probe());
    let b = a.clone();
    assert!(matches!(
        b.probe(),
        systemprompt_mcp::services::health_probe::HealthProbe::Handshake(_)
    ));
}
