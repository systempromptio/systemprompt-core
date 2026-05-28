use systemprompt_agent::services::agent_orchestration::monitor::{
    HealthCheckResult, MonitoringReport, check_a2a_agent_health, check_agent_health,
    check_agent_responsiveness,
};

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
fn monitoring_report_total_sums_buckets() {
    let mut report = MonitoringReport::new();
    report.healthy.push("a".to_string());
    report.healthy.push("b".to_string());
    report.unhealthy.push("c".to_string());
    report.failed.push("d".to_string());
    assert_eq!(report.total_agents(), 4);
}

#[test]
fn monitoring_report_percentage_zero_when_empty() {
    let report = MonitoringReport::new();
    assert_eq!(report.healthy_percentage(), 0.0);
}

#[test]
fn monitoring_report_percentage_all_healthy() {
    let mut report = MonitoringReport::new();
    report.healthy.push("a".to_string());
    report.healthy.push("b".to_string());
    assert!((report.healthy_percentage() - 100.0).abs() < f64::EPSILON);
}

#[test]
fn monitoring_report_percentage_half() {
    let mut report = MonitoringReport::new();
    report.healthy.push("a".to_string());
    report.unhealthy.push("b".to_string());
    assert!((report.healthy_percentage() - 50.0).abs() < f64::EPSILON);
}

#[test]
fn monitoring_report_debug() {
    let report = MonitoringReport::new();
    assert!(format!("{:?}", report).contains("MonitoringReport"));
}

#[tokio::test]
async fn check_agent_health_returns_unhealthy_for_unreachable_port() {
    // get_agent_port_simple("monitorless-test") yields a port unlikely to
    // have a listener; either way the TCP connect should yield an
    // unhealthy/timeout result, not an error.
    let res = check_agent_health("monitorless-test").await;
    assert!(res.is_ok());
    let r: HealthCheckResult = res.unwrap();
    assert!(!r.healthy);
}

#[tokio::test]
async fn check_agent_responsiveness_returns_false_for_no_listener() {
    let res = check_agent_responsiveness("nothing-listening", 1).await;
    assert!(res.is_ok());
    assert!(!res.unwrap());
}

#[tokio::test]
async fn check_a2a_agent_health_returns_false_for_no_listener() {
    // Choose a port that definitely has nothing listening.
    let res = check_a2a_agent_health(1, 1).await;
    assert!(res.is_ok());
    assert!(!res.unwrap());
}

#[tokio::test]
async fn check_agent_health_handles_numeric_name() {
    let res = check_agent_health("agent42").await;
    assert!(res.is_ok());
}
