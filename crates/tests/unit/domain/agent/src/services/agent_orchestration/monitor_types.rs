use systemprompt_agent::services::agent_orchestration::monitor::HealthCheckResult;

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

