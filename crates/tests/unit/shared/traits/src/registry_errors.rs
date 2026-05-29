//! Tests for registry.rs, scheduler.rs, process.rs, and session_analytics error types.

use systemprompt_traits::registry::{
    AgentInfo, McpServerInfo, RegistryError, ServiceOAuthConfig,
};
use systemprompt_traits::scheduler::{JobInfo, JobStatus, SchedulerError};
use systemprompt_traits::process::{ProcessProviderError, ProcessResult};
use systemprompt_traits::session_analytics::{
    SessionAnalyticsProviderError, SessionAnalyticsResult,
};
use systemprompt_traits::mcp_service::{McpServerMetadata, McpServiceProviderError, McpServiceResult};
use systemprompt_traits::ai_providers::AiProviderError;

// --- RegistryError display ---

#[test]
fn registry_not_found_display() {
    let e = RegistryError::NotFound("my-agent".to_owned());
    assert!(format!("{e}").contains("my-agent"));
}

#[test]
fn registry_unavailable_display() {
    let e = RegistryError::Unavailable("no connection".to_owned());
    assert!(format!("{e}").contains("no connection"));
}

#[test]
fn registry_configuration_display() {
    let e = RegistryError::Configuration("bad port".to_owned());
    assert!(format!("{e}").contains("bad port"));
}

#[test]
fn registry_internal_display() {
    let e = RegistryError::Internal("panic".to_owned());
    assert!(format!("{e}").contains("panic"));
}

#[test]
fn registry_errors_are_debug() {
    let variants: &[RegistryError] = &[
        RegistryError::NotFound("a".into()),
        RegistryError::Unavailable("b".into()),
        RegistryError::Configuration("c".into()),
        RegistryError::Internal("d".into()),
    ];
    for e in variants {
        assert!(!format!("{e:?}").is_empty());
    }
}

// --- ServiceOAuthConfig ---

#[test]
fn service_oauth_config_default_required_true() {
    let c = ServiceOAuthConfig::default();
    assert!(c.required);
    assert!(c.scopes.is_empty());
    assert!(c.audience.is_empty());
}

#[test]
fn service_oauth_config_clone() {
    let c = ServiceOAuthConfig {
        required: false,
        scopes: vec!["read".into(), "write".into()],
        audience: "api".into(),
    };
    let c2 = c.clone();
    assert!(!c2.required);
    assert_eq!(c2.scopes, vec!["read", "write"]);
    assert_eq!(c2.audience, "api");
}

// --- AgentInfo ---

#[test]
fn agent_info_fields_accessible() {
    let a = AgentInfo {
        name: "my-agent".to_owned(),
        port: 9000,
        enabled: true,
        oauth: ServiceOAuthConfig::default(),
    };
    assert_eq!(a.name, "my-agent");
    assert_eq!(a.port, 9000);
    assert!(a.enabled);
}

#[test]
fn agent_info_clone() {
    let a = AgentInfo {
        name: "agent-x".to_owned(),
        port: 1234,
        enabled: false,
        oauth: ServiceOAuthConfig::default(),
    };
    let b = a.clone();
    assert_eq!(b.name, "agent-x");
    assert_eq!(b.port, 1234);
}

// --- McpServerInfo ---

#[test]
fn mcp_server_info_fields_accessible() {
    let s = McpServerInfo {
        name: "mcp-server".to_owned(),
        port: 3000,
        enabled: true,
        oauth: ServiceOAuthConfig::default(),
    };
    assert_eq!(s.name, "mcp-server");
    assert_eq!(s.port, 3000);
}

#[test]
fn mcp_server_info_clone() {
    let s = McpServerInfo {
        name: "svc".to_owned(),
        port: 8888,
        enabled: false,
        oauth: ServiceOAuthConfig::default(),
    };
    let t = s.clone();
    assert_eq!(t.name, "svc");
}

// --- SchedulerError display ---

#[test]
fn scheduler_job_not_found_display() {
    let e = SchedulerError::JobNotFound("sync-job".to_owned());
    assert!(format!("{e}").contains("sync-job"));
}

#[test]
fn scheduler_unavailable_display() {
    let e = SchedulerError::Unavailable("no worker".to_owned());
    assert!(format!("{e}").contains("no worker"));
}

#[test]
fn scheduler_execution_failed_display() {
    let e = SchedulerError::ExecutionFailed("timeout".to_owned());
    assert!(format!("{e}").contains("timeout"));
}

#[test]
fn scheduler_internal_display() {
    let e = SchedulerError::Internal("oops".to_owned());
    assert!(format!("{e}").contains("oops"));
}

// --- JobStatus ---

#[test]
fn job_status_eq() {
    assert_eq!(JobStatus::Pending, JobStatus::Pending);
    assert_ne!(JobStatus::Pending, JobStatus::Running);
    assert_ne!(JobStatus::Success, JobStatus::Failed);
    assert_ne!(JobStatus::Disabled, JobStatus::Success);
}

#[test]
fn job_status_copy() {
    let s = JobStatus::Running;
    let t = s;
    assert_eq!(s, t);
}

#[test]
fn job_status_debug() {
    assert_eq!(format!("{:?}", JobStatus::Pending), "Pending");
    assert_eq!(format!("{:?}", JobStatus::Running), "Running");
    assert_eq!(format!("{:?}", JobStatus::Success), "Success");
    assert_eq!(format!("{:?}", JobStatus::Failed), "Failed");
    assert_eq!(format!("{:?}", JobStatus::Disabled), "Disabled");
}

// --- JobInfo ---

#[test]
fn job_info_fields_accessible() {
    let j = JobInfo {
        name: "cleanup".to_owned(),
        status: JobStatus::Success,
        last_run: None,
        next_run: None,
        run_count: 42,
        last_error: None,
    };
    assert_eq!(j.name, "cleanup");
    assert_eq!(j.status, JobStatus::Success);
    assert_eq!(j.run_count, 42);
}

#[test]
fn job_info_clone() {
    let j = JobInfo {
        name: "job".to_owned(),
        status: JobStatus::Failed,
        last_run: None,
        next_run: None,
        run_count: 1,
        last_error: Some("err msg".to_owned()),
    };
    let j2 = j.clone();
    assert_eq!(j2.last_error.as_deref(), Some("err msg"));
}

// --- ProcessProviderError display ---

#[test]
fn process_not_found_display_contains_pid() {
    let e = ProcessProviderError::NotFound(12345);
    assert!(format!("{e}").contains("12345"));
}

#[test]
fn process_operation_failed_display() {
    let e = ProcessProviderError::OperationFailed("kill failed".to_owned());
    assert!(format!("{e}").contains("kill failed"));
}

#[test]
fn process_port_timeout_display_contains_port() {
    let e = ProcessProviderError::PortTimeout(8080);
    assert!(format!("{e}").contains("8080"));
}

#[test]
fn process_internal_display() {
    let e = ProcessProviderError::Internal("io error".to_owned());
    assert!(format!("{e}").contains("io error"));
}

#[test]
fn process_result_alias_roundtrip() {
    let r: ProcessResult<u32> = Ok(99);
    assert_eq!(r.unwrap(), 99);
    let r: ProcessResult<()> = Err(ProcessProviderError::Internal("x".into()));
    assert!(r.is_err());
}

// --- SessionAnalyticsProviderError ---

#[test]
fn session_analytics_not_found_display() {
    let e = SessionAnalyticsProviderError::SessionNotFound;
    let s = format!("{e}");
    assert!(!s.is_empty());
}

#[test]
fn session_analytics_internal_display() {
    let e = SessionAnalyticsProviderError::Internal("db gone".to_owned());
    assert!(format!("{e}").contains("db gone"));
}

#[test]
fn session_analytics_result_ok() {
    let r: SessionAnalyticsResult<i32> = Ok(1);
    assert_eq!(r.unwrap(), 1);
}

// --- McpServiceProviderError display ---

#[test]
fn mcp_service_server_not_found_display() {
    let e = McpServiceProviderError::ServerNotFound("tools-server".to_owned());
    assert!(format!("{e}").contains("tools-server"));
}

#[test]
fn mcp_service_registry_unavailable_display() {
    let e = McpServiceProviderError::RegistryUnavailable;
    let s = format!("{e}");
    assert!(!s.is_empty());
}

#[test]
fn mcp_service_internal_display() {
    let e = McpServiceProviderError::Internal("timeout".to_owned());
    assert!(format!("{e}").contains("timeout"));
}

#[test]
fn mcp_service_result_alias() {
    let r: McpServiceResult<()> = Ok(());
    assert!(r.is_ok());
}

// --- McpServerMetadata ---

#[test]
fn mcp_server_metadata_fields() {
    let m = McpServerMetadata {
        name: "myserver".to_owned(),
        endpoint: "http://localhost:3000".to_owned(),
    };
    assert_eq!(m.name, "myserver");
    assert_eq!(m.endpoint, "http://localhost:3000");
}

#[test]
fn mcp_server_metadata_clone() {
    let m = McpServerMetadata {
        name: "a".to_owned(),
        endpoint: "b".to_owned(),
    };
    let m2 = m.clone();
    assert_eq!(m2.name, "a");
}

// --- AiProviderError display ---

#[test]
fn ai_provider_file_not_found_display() {
    let e = AiProviderError::FileNotFound("img.png".to_owned());
    assert!(format!("{e}").contains("img.png"));
}

#[test]
fn ai_provider_session_not_found_display() {
    let e = AiProviderError::SessionNotFound("sess-123".to_owned());
    assert!(format!("{e}").contains("sess-123"));
}

#[test]
fn ai_provider_storage_error_display() {
    let e = AiProviderError::StorageError("disk full".to_owned());
    assert!(format!("{e}").contains("disk full"));
}

#[test]
fn ai_provider_configuration_error_display() {
    let e = AiProviderError::ConfigurationError("missing key".to_owned());
    assert!(format!("{e}").contains("missing key"));
}

#[test]
fn ai_provider_internal_display() {
    let e = AiProviderError::Internal("unexpected".to_owned());
    assert!(format!("{e}").contains("unexpected"));
}
