use systemprompt_agent::services::agent_orchestration::{
    AgentRuntimeConfig, AgentStatus, OrchestrationError, ValidationReport,
};
use systemprompt_agent::services::agent_orchestration::orchestrator::AgentInfo;

#[test]
fn test_agent_status_running_eq() {
    let a = AgentStatus::Running { pid: 1, port: 80 };
    let b = AgentStatus::Running { pid: 1, port: 80 };

    assert_eq!(a, b);
}

#[test]
fn test_agent_status_running_ne_pid() {
    let a = AgentStatus::Running { pid: 1, port: 80 };
    let b = AgentStatus::Running { pid: 2, port: 80 };

    assert_ne!(a, b);
}

#[test]
fn test_agent_status_running_ne_port() {
    let a = AgentStatus::Running {
        pid: 1,
        port: 8080,
    };
    let b = AgentStatus::Running {
        pid: 1,
        port: 9090,
    };

    assert_ne!(a, b);
}

#[test]
fn test_agent_status_failed_eq() {
    let a = AgentStatus::Failed {
        reason: "err".to_string(),
        last_attempt: None,
        retry_count: 0,
    };
    let b = AgentStatus::Failed {
        reason: "err".to_string(),
        last_attempt: None,
        retry_count: 0,
    };

    assert_eq!(a, b);
}

#[test]
fn test_agent_status_failed_ne_reason() {
    let a = AgentStatus::Failed {
        reason: "err1".to_string(),
        last_attempt: None,
        retry_count: 0,
    };
    let b = AgentStatus::Failed {
        reason: "err2".to_string(),
        last_attempt: None,
        retry_count: 0,
    };

    assert_ne!(a, b);
}

#[test]
fn test_agent_status_running_ne_failed() {
    let running = AgentStatus::Running { pid: 1, port: 80 };
    let failed = AgentStatus::Failed {
        reason: "x".to_string(),
        last_attempt: None,
        retry_count: 0,
    };

    assert_ne!(running, failed);
}

#[test]
fn test_agent_status_clone_running() {
    let status = AgentStatus::Running {
        pid: 55,
        port: 4000,
    };

    let cloned = status.clone();
    assert_eq!(cloned, status);
}

#[test]
fn test_agent_status_clone_failed() {
    let status = AgentStatus::Failed {
        reason: "clone reason".to_string(),
        last_attempt: Some("2026-01-01".to_string()),
        retry_count: 5,
    };

    let cloned = status.clone();
    assert_eq!(cloned, status);
}

#[test]
fn test_validation_report_default_equals_new() {
    let default_report = ValidationReport::default();
    let new_report = ValidationReport::new();

    assert_eq!(default_report.valid, new_report.valid);
    assert_eq!(default_report.issues.len(), new_report.issues.len());
}

#[test]
fn test_agent_runtime_config_various_ports() {
    for port in [0u16, 80, 443, 8080, 65535] {
        let config = AgentRuntimeConfig {
            id: format!("port-{}", port),
            name: format!("Agent on port {}", port),
            port,
        };
        assert_eq!(config.port, port);
    }
}

#[test]
fn test_agent_info_construction() {
    let info = AgentInfo {
        id: "info-1".to_string(),
        name: "Test Agent".to_string(),
        status: AgentStatus::Running {
            pid: 100,
            port: 8080,
        },
        port: 8080,
    };

    assert_eq!(info.id, "info-1");
    assert_eq!(info.name, "Test Agent");
    assert_eq!(info.port, 8080);
}

#[test]
fn test_agent_info_with_failed_status() {
    let info = AgentInfo {
        id: "info-2".to_string(),
        name: "Failed Agent".to_string(),
        status: AgentStatus::Failed {
            reason: "crashed".to_string(),
            last_attempt: None,
            retry_count: 1,
        },
        port: 9090,
    };

    assert_eq!(info.name, "Failed Agent");
    assert_eq!(info.port, 9090);
}

#[test]
fn test_agent_info_clone() {
    let info = AgentInfo {
        id: "clone-info".to_string(),
        name: "Cloned".to_string(),
        status: AgentStatus::Running {
            pid: 200,
            port: 3000,
        },
        port: 3000,
    };

    let cloned = info.clone();
    assert_eq!(cloned.id, "clone-info");
    assert_eq!(cloned.name, "Cloned");
    assert_eq!(cloned.port, 3000);
}

#[test]
fn test_agent_info_debug() {
    let info = AgentInfo {
        id: "debug-info".to_string(),
        name: "Debug".to_string(),
        status: AgentStatus::Running {
            pid: 300,
            port: 5000,
        },
        port: 5000,
    };

    let debug_str = format!("{:?}", info);
    assert!(debug_str.contains("AgentInfo"));
    assert!(debug_str.contains("debug-info"));
}

#[test]
fn test_orchestration_error_io_from() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let orch_err: OrchestrationError = io_err.into();

    assert!(orch_err.to_string().contains("file not found"));
}

#[test]
fn test_orchestration_error_generic_from_anyhow() {
    let anyhow_err = anyhow::anyhow!("something went wrong");
    let orch_err: OrchestrationError = anyhow_err.into();

    assert!(orch_err.to_string().contains("something went wrong"));
}
