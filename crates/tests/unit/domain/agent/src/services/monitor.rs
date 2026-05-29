use systemprompt_agent::services::agent_orchestration::monitor::{
    HealthCheckResult, MonitoringReport,
};

#[test]
fn health_check_result_healthy() {
    let result = HealthCheckResult {
        healthy: true,
        message: "TCP connection successful".to_string(),
        response_time_ms: 5,
    };
    assert!(result.healthy);
    assert_eq!(result.message, "TCP connection successful");
    assert_eq!(result.response_time_ms, 5);
}

#[test]
fn health_check_result_unhealthy() {
    let result = HealthCheckResult {
        healthy: false,
        message: "Connection refused".to_string(),
        response_time_ms: 0,
    };
    assert!(!result.healthy);
    assert!(result.message.contains("refused"));
}

#[test]
fn health_check_result_debug() {
    let result = HealthCheckResult {
        healthy: true,
        message: "ok".to_string(),
        response_time_ms: 10,
    };
    let dbg = format!("{:?}", result);
    assert!(dbg.contains("HealthCheckResult"));
    assert!(dbg.contains("true"));
}

#[test]
fn health_check_result_clone() {
    let result = HealthCheckResult {
        healthy: false,
        message: "timeout".to_string(),
        response_time_ms: 5000,
    };
    let cloned = result.clone();
    assert_eq!(cloned.healthy, result.healthy);
    assert_eq!(cloned.message, result.message);
    assert_eq!(cloned.response_time_ms, result.response_time_ms);
}

#[test]
fn monitoring_report_new_is_empty() {
    let report = MonitoringReport::new();
    assert!(report.healthy.is_empty());
    assert!(report.unhealthy.is_empty());
    assert!(report.failed.is_empty());
    assert_eq!(report.total_agents(), 0);
}

#[test]
fn monitoring_report_default_matches_new() {
    let report = MonitoringReport::default();
    assert_eq!(report.total_agents(), 0);
}

#[test]
fn monitoring_report_total_agents_counts_all() {
    let mut report = MonitoringReport::new();
    report.healthy.push("a1".to_string());
    report.healthy.push("a2".to_string());
    report.unhealthy.push("a3".to_string());
    report.failed.push("a4".to_string());
    report.failed.push("a5".to_string());
    assert_eq!(report.total_agents(), 5);
}

#[test]
fn monitoring_report_healthy_percentage_all_healthy() {
    let mut report = MonitoringReport::new();
    report.healthy.push("a1".to_string());
    report.healthy.push("a2".to_string());
    report.healthy.push("a3".to_string());
    assert!((report.healthy_percentage() - 100.0).abs() < f64::EPSILON);
}

#[test]
fn monitoring_report_healthy_percentage_none_healthy() {
    let mut report = MonitoringReport::new();
    report.failed.push("a1".to_string());
    report.failed.push("a2".to_string());
    assert!((report.healthy_percentage() - 0.0).abs() < f64::EPSILON);
}

#[test]
fn monitoring_report_healthy_percentage_mixed() {
    let mut report = MonitoringReport::new();
    report.healthy.push("a1".to_string());
    report.healthy.push("a2".to_string());
    report.failed.push("a3".to_string());
    report.failed.push("a4".to_string());
    assert!((report.healthy_percentage() - 50.0).abs() < f64::EPSILON);
}

#[test]
fn monitoring_report_healthy_percentage_zero_agents() {
    let report = MonitoringReport::new();
    assert!((report.healthy_percentage() - 0.0).abs() < f64::EPSILON);
}

#[test]
fn monitoring_report_debug() {
    let mut report = MonitoringReport::new();
    report.healthy.push("agent1".to_string());
    let dbg = format!("{:?}", report);
    assert!(dbg.contains("MonitoringReport"));
    assert!(dbg.contains("agent1"));
}

#[test]
fn monitoring_report_one_of_three_healthy() {
    let mut report = MonitoringReport::new();
    report.healthy.push("h1".to_string());
    report.unhealthy.push("u1".to_string());
    report.failed.push("f1".to_string());
    assert_eq!(report.total_agents(), 3);
    let pct = report.healthy_percentage();
    assert!((pct - 33.333_333_333_333_336).abs() < 1.0);
}
