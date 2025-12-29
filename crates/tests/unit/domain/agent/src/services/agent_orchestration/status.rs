//! Unit tests for agent orchestration status types
//!
//! Tests cover:
//! - AgentStatus variants
//! - AgentRuntimeConfig
//! - ValidationReport
//! - OrchestrationError variants

use systemprompt_core_agent::services::agent_orchestration::{
    AgentRuntimeConfig, AgentStatus, OrchestrationError, ValidationReport,
};

// ============================================================================
// AgentStatus Tests
// ============================================================================

#[test]
fn test_agent_status_running() {
    let status = AgentStatus::Running {
        pid: 12345,
        port: 8080,
    };

    match status {
        AgentStatus::Running { pid, port } => {
            assert_eq!(pid, 12345);
            assert_eq!(port, 8080);
        }
        _ => panic!("Expected Running variant"),
    }
}

#[test]
fn test_agent_status_failed() {
    let status = AgentStatus::Failed {
        reason: "Connection refused".to_string(),
        last_attempt: Some("2024-01-01T12:00:00Z".to_string()),
        retry_count: 3,
    };

    match status {
        AgentStatus::Failed {
            reason,
            last_attempt,
            retry_count,
        } => {
            assert_eq!(reason, "Connection refused");
            assert_eq!(last_attempt, Some("2024-01-01T12:00:00Z".to_string()));
            assert_eq!(retry_count, 3);
        }
        _ => panic!("Expected Failed variant"),
    }
}

#[test]
fn test_agent_status_failed_no_last_attempt() {
    let status = AgentStatus::Failed {
        reason: "First failure".to_string(),
        last_attempt: None,
        retry_count: 1,
    };

    match status {
        AgentStatus::Failed { last_attempt, .. } => {
            assert!(last_attempt.is_none());
        }
        _ => panic!("Expected Failed variant"),
    }
}

#[test]
fn test_agent_status_debug_running() {
    let status = AgentStatus::Running {
        pid: 1000,
        port: 9000,
    };

    let debug_str = format!("{:?}", status);
    assert!(debug_str.contains("Running"));
    assert!(debug_str.contains("1000"));
    assert!(debug_str.contains("9000"));
}

#[test]
fn test_agent_status_debug_failed() {
    let status = AgentStatus::Failed {
        reason: "Test reason".to_string(),
        last_attempt: None,
        retry_count: 0,
    };

    let debug_str = format!("{:?}", status);
    assert!(debug_str.contains("Failed"));
    assert!(debug_str.contains("Test reason"));
}

#[test]
fn test_agent_status_clone() {
    let status = AgentStatus::Running {
        pid: 5555,
        port: 7777,
    };

    let cloned = status.clone();
    assert_eq!(status, cloned);
}

#[test]
fn test_agent_status_equality() {
    let status1 = AgentStatus::Running {
        pid: 100,
        port: 200,
    };
    let status2 = AgentStatus::Running {
        pid: 100,
        port: 200,
    };

    assert_eq!(status1, status2);
}

#[test]
fn test_agent_status_inequality() {
    let status1 = AgentStatus::Running {
        pid: 100,
        port: 200,
    };
    let status2 = AgentStatus::Running {
        pid: 100,
        port: 300,
    };

    assert_ne!(status1, status2);
}

// ============================================================================
// AgentRuntimeConfig Tests
// ============================================================================

#[test]
fn test_agent_runtime_config_creation() {
    let config = AgentRuntimeConfig {
        id: "config-1".to_string(),
        name: "Test Agent".to_string(),
        port: 8080,
    };

    assert_eq!(config.id, "config-1");
    assert_eq!(config.name, "Test Agent");
    assert_eq!(config.port, 8080);
}

#[test]
fn test_agent_runtime_config_debug() {
    let config = AgentRuntimeConfig {
        id: "debug-config".to_string(),
        name: "Debug Agent".to_string(),
        port: 3000,
    };

    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("AgentRuntimeConfig"));
    assert!(debug_str.contains("debug-config"));
    assert!(debug_str.contains("Debug Agent"));
    assert!(debug_str.contains("3000"));
}

#[test]
fn test_agent_runtime_config_clone() {
    let config = AgentRuntimeConfig {
        id: "clone-config".to_string(),
        name: "Clone Agent".to_string(),
        port: 4000,
    };

    let cloned = config.clone();
    assert_eq!(cloned.id, "clone-config");
    assert_eq!(cloned.name, "Clone Agent");
    assert_eq!(cloned.port, 4000);
}

// ============================================================================
// ValidationReport Tests
// ============================================================================

#[test]
fn test_validation_report_new() {
    let report = ValidationReport::new();

    assert!(report.valid);
    assert!(report.issues.is_empty());
}

#[test]
fn test_validation_report_with_issue() {
    let report = ValidationReport::with_issue("Port already in use".to_string());

    assert!(!report.valid);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(report.issues[0], "Port already in use");
}

#[test]
fn test_validation_report_add_issue() {
    let mut report = ValidationReport::new();

    assert!(report.valid);

    report.add_issue("First issue".to_string());

    assert!(!report.valid);
    assert_eq!(report.issues.len(), 1);
}

#[test]
fn test_validation_report_add_multiple_issues() {
    let mut report = ValidationReport::new();

    report.add_issue("Issue 1".to_string());
    report.add_issue("Issue 2".to_string());
    report.add_issue("Issue 3".to_string());

    assert!(!report.valid);
    assert_eq!(report.issues.len(), 3);
    assert!(report.issues.contains(&"Issue 1".to_string()));
    assert!(report.issues.contains(&"Issue 2".to_string()));
    assert!(report.issues.contains(&"Issue 3".to_string()));
}

#[test]
fn test_validation_report_stays_invalid() {
    let mut report = ValidationReport::with_issue("Initial issue".to_string());

    assert!(!report.valid);

    report.add_issue("Another issue".to_string());

    assert!(!report.valid);
    assert_eq!(report.issues.len(), 2);
}

// ============================================================================
// OrchestrationError Tests
// ============================================================================

#[test]
fn test_orchestration_error_agent_not_found() {
    let error = OrchestrationError::AgentNotFound("missing-agent".to_string());

    assert!(error.to_string().contains("Agent"));
    assert!(error.to_string().contains("not found"));
    assert!(error.to_string().contains("missing-agent"));
}

#[test]
fn test_orchestration_error_agent_already_running() {
    let error = OrchestrationError::AgentAlreadyRunning("running-agent".to_string());

    assert!(error.to_string().contains("already running"));
    assert!(error.to_string().contains("running-agent"));
}

#[test]
fn test_orchestration_error_process_spawn_failed() {
    let error = OrchestrationError::ProcessSpawnFailed("Failed to bind to port 8080".to_string());

    assert!(error.to_string().contains("Process spawn failed"));
    assert!(error.to_string().contains("port 8080"));
}

#[test]
fn test_orchestration_error_database() {
    let error = OrchestrationError::Database("Connection lost".to_string());

    assert!(error.to_string().contains("Database error"));
    assert!(error.to_string().contains("Connection lost"));
}

#[test]
fn test_orchestration_error_health_check_timeout() {
    let error = OrchestrationError::HealthCheckTimeout("slow-agent".to_string());

    assert!(error.to_string().contains("Health check timeout"));
    assert!(error.to_string().contains("slow-agent"));
}

#[test]
fn test_orchestration_error_debug() {
    let error = OrchestrationError::AgentNotFound("test-agent".to_string());
    let debug_str = format!("{:?}", error);

    assert!(debug_str.contains("AgentNotFound"));
    assert!(debug_str.contains("test-agent"));
}

// ============================================================================
// OrchestrationResult Tests
// ============================================================================

#[test]
fn test_orchestration_result_ok() {
    let result: Result<i32, OrchestrationError> = Ok(42);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);
}

#[test]
fn test_orchestration_result_err() {
    let result: Result<i32, OrchestrationError> =
        Err(OrchestrationError::AgentNotFound("test".to_string()));
    assert!(result.is_err());
}
