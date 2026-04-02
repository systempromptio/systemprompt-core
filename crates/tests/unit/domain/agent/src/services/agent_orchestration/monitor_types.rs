use systemprompt_agent::services::agent_orchestration::monitor::{
    HealthCheckResult, MonitoringReport,
};

#[test]
fn test_health_check_result_healthy() {
    let result = HealthCheckResult {
        healthy: true,
        message: "TCP connection successful".to_string(),
        response_time_ms: 42,
    };

    assert!(result.healthy);
    assert_eq!(result.message, "TCP connection successful");
    assert_eq!(result.response_time_ms, 42);
}

#[test]
fn test_health_check_result_unhealthy() {
    let result = HealthCheckResult {
        healthy: false,
        message: "Connection refused".to_string(),
        response_time_ms: 0,
    };

    assert!(!result.healthy);
    assert_eq!(result.response_time_ms, 0);
}

#[test]
fn test_health_check_result_clone() {
    let result = HealthCheckResult {
        healthy: true,
        message: "OK".to_string(),
        response_time_ms: 100,
    };

    let cloned = result.clone();
    assert_eq!(cloned.healthy, result.healthy);
    assert_eq!(cloned.message, result.message);
    assert_eq!(cloned.response_time_ms, result.response_time_ms);
}

#[test]
fn test_health_check_result_debug() {
    let result = HealthCheckResult {
        healthy: true,
        message: "debug-test".to_string(),
        response_time_ms: 5,
    };

    let debug_str = format!("{:?}", result);
    assert!(debug_str.contains("HealthCheckResult"));
    assert!(debug_str.contains("debug-test"));
}

#[test]
fn test_monitoring_report_new() {
    let report = MonitoringReport::new();

    assert!(report.healthy.is_empty());
    assert!(report.unhealthy.is_empty());
    assert!(report.failed.is_empty());
}

#[test]
fn test_monitoring_report_default() {
    let report = MonitoringReport::default();

    assert!(report.healthy.is_empty());
    assert!(report.unhealthy.is_empty());
    assert!(report.failed.is_empty());
}

#[test]
fn test_monitoring_report_total_agents_empty() {
    let report = MonitoringReport::new();
    assert_eq!(report.total_agents(), 0);
}

#[test]
fn test_monitoring_report_total_agents_mixed() {
    let mut report = MonitoringReport::new();
    report.healthy.push("agent-1".to_string());
    report.healthy.push("agent-2".to_string());
    report.unhealthy.push("agent-3".to_string());
    report.failed.push("agent-4".to_string());

    assert_eq!(report.total_agents(), 4);
}

#[test]
fn test_monitoring_report_healthy_percentage_empty() {
    let report = MonitoringReport::new();
    assert_eq!(report.healthy_percentage(), 0.0);
}

#[test]
fn test_monitoring_report_healthy_percentage_all_healthy() {
    let mut report = MonitoringReport::new();
    report.healthy.push("agent-1".to_string());
    report.healthy.push("agent-2".to_string());

    assert!((report.healthy_percentage() - 100.0).abs() < f64::EPSILON);
}

#[test]
fn test_monitoring_report_healthy_percentage_half() {
    let mut report = MonitoringReport::new();
    report.healthy.push("agent-1".to_string());
    report.failed.push("agent-2".to_string());

    assert!((report.healthy_percentage() - 50.0).abs() < f64::EPSILON);
}

#[test]
fn test_monitoring_report_healthy_percentage_none_healthy() {
    let mut report = MonitoringReport::new();
    report.unhealthy.push("agent-1".to_string());
    report.failed.push("agent-2".to_string());

    assert!((report.healthy_percentage() - 0.0).abs() < f64::EPSILON);
}

#[test]
fn test_monitoring_report_total_agents_only_healthy() {
    let mut report = MonitoringReport::new();
    report.healthy.push("a".to_string());
    report.healthy.push("b".to_string());
    report.healthy.push("c".to_string());

    assert_eq!(report.total_agents(), 3);
}

#[test]
fn test_monitoring_report_total_agents_only_failed() {
    let mut report = MonitoringReport::new();
    report.failed.push("a".to_string());

    assert_eq!(report.total_agents(), 1);
}
