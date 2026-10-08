use systemprompt_agent::services::agent_orchestration::monitor::HealthCheckResult;

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
