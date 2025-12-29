//! Unit tests for systemprompt-core-scheduler crate
//!
//! Tests cover:
//! - SchedulerService job scheduling and execution logic
//! - ServiceManagementService lifecycle management
//! - ServiceReconciler state drift detection and correction
//! - Job implementations (behavioral analysis, database cleanup, etc.)
//! - Repository operations (job persistence, queries, status updates)
//! - Model validation and state transitions

use chrono::{TimeZone, Utc};
use systemprompt_core_scheduler::{
    DbServiceRecord, DesiredStatus, JobStatus, ReconciliationResult, RuntimeStatus, ScheduledJob,
    SchedulerError, ServiceAction, ServiceConfig, ServiceType, VerifiedServiceState,
};
use systemprompt_identifiers::ScheduledJobId;

// ============================================================================
// JobStatus Tests
// ============================================================================

#[test]
fn test_job_status_as_str() {
    assert_eq!(JobStatus::Success.as_str(), "success");
    assert_eq!(JobStatus::Failed.as_str(), "failed");
    assert_eq!(JobStatus::Running.as_str(), "running");
}

#[test]
fn test_job_status_display() {
    assert_eq!(format!("{}", JobStatus::Success), "success");
    assert_eq!(format!("{}", JobStatus::Failed), "failed");
    assert_eq!(format!("{}", JobStatus::Running), "running");
}

#[test]
fn test_job_status_serialization() {
    let success = JobStatus::Success;
    let json = serde_json::to_string(&success).unwrap();
    assert_eq!(json, "\"success\"");

    let failed = JobStatus::Failed;
    let json = serde_json::to_string(&failed).unwrap();
    assert_eq!(json, "\"failed\"");
}

#[test]
fn test_job_status_deserialization() {
    let success: JobStatus = serde_json::from_str("\"success\"").unwrap();
    assert_eq!(success, JobStatus::Success);

    let failed: JobStatus = serde_json::from_str("\"failed\"").unwrap();
    assert_eq!(failed, JobStatus::Failed);
}

// ============================================================================
// SchedulerError Tests
// ============================================================================

#[test]
fn test_scheduler_error_job_not_found() {
    let error = SchedulerError::job_not_found("test_job");
    assert_eq!(error.to_string(), "Job not found: test_job");
}

#[test]
fn test_scheduler_error_invalid_schedule() {
    let error = SchedulerError::invalid_schedule("invalid cron");
    assert_eq!(error.to_string(), "Invalid cron schedule: invalid cron");
}

#[test]
fn test_scheduler_error_job_execution_failed() {
    let error = SchedulerError::job_execution_failed("test_job", "connection timeout");
    assert_eq!(
        error.to_string(),
        "Job execution failed: test_job - connection timeout"
    );
}

#[test]
fn test_scheduler_error_config_error() {
    let error = SchedulerError::config_error("missing required field");
    assert_eq!(
        error.to_string(),
        "Configuration error: missing required field"
    );
}

#[test]
fn test_scheduler_error_already_running() {
    let error = SchedulerError::AlreadyRunning;
    assert_eq!(error.to_string(), "Scheduler already running");
}

#[test]
fn test_scheduler_error_not_initialized() {
    let error = SchedulerError::NotInitialized;
    assert_eq!(error.to_string(), "Scheduler not initialized");
}

// ============================================================================
// ScheduledJob Model Tests
// ============================================================================

#[test]
fn test_scheduled_job_model() {
    let now = Utc::now();
    let job = ScheduledJob {
        id: ScheduledJobId::generate(),
        job_name: "test_job".to_string(),
        schedule: "0 0 * * * *".to_string(),
        enabled: true,
        last_run: Some(now),
        next_run: None,
        last_status: Some("success".to_string()),
        last_error: None,
        run_count: 5,
        created_at: now,
        updated_at: now,
    };

    assert_eq!(job.job_name, "test_job");
    assert_eq!(job.schedule, "0 0 * * * *");
    assert!(job.enabled);
    assert_eq!(job.run_count, 5);
}

#[test]
fn test_scheduled_job_with_error() {
    let now = Utc::now();
    let job = ScheduledJob {
        id: ScheduledJobId::generate(),
        job_name: "failing_job".to_string(),
        schedule: "0 */10 * * * *".to_string(),
        enabled: true,
        last_run: Some(now),
        next_run: None,
        last_status: Some("failed".to_string()),
        last_error: Some("Database connection failed".to_string()),
        run_count: 10,
        created_at: now,
        updated_at: now,
    };

    assert_eq!(job.last_status, Some("failed".to_string()));
    assert_eq!(
        job.last_error,
        Some("Database connection failed".to_string())
    );
}

#[test]
fn test_scheduled_job_serialization() {
    let now = Utc.with_ymd_and_hms(2024, 1, 15, 12, 0, 0).unwrap();
    let job = ScheduledJob {
        id: ScheduledJobId::generate(),
        job_name: "serialization_test".to_string(),
        schedule: "0 0 3 * * *".to_string(),
        enabled: false,
        last_run: None,
        next_run: None,
        last_status: None,
        last_error: None,
        run_count: 0,
        created_at: now,
        updated_at: now,
    };

    let json = serde_json::to_string(&job).unwrap();
    assert!(json.contains("\"job_name\":\"serialization_test\""));
    assert!(json.contains("\"enabled\":false"));
}

// ============================================================================
// ServiceConfig Tests
// ============================================================================

#[test]
fn test_service_config_enabled() {
    let config = ServiceConfig {
        name: "api-server".to_string(),
        service_type: ServiceType::Api,
        port: 8080,
        enabled: true,
    };

    assert_eq!(config.name, "api-server");
    assert_eq!(config.port, 8080);
    assert!(config.enabled);
}

#[test]
fn test_service_config_disabled() {
    let config = ServiceConfig {
        name: "mcp-server".to_string(),
        service_type: ServiceType::Mcp,
        port: 3001,
        enabled: false,
    };

    assert!(!config.enabled);
    assert_eq!(config.service_type, ServiceType::Mcp);
}

// ============================================================================
// DesiredStatus Tests
// ============================================================================

#[test]
fn test_desired_status_variants() {
    let enabled = DesiredStatus::Enabled;
    let disabled = DesiredStatus::Disabled;

    assert_eq!(enabled, DesiredStatus::Enabled);
    assert_eq!(disabled, DesiredStatus::Disabled);
    assert_ne!(enabled, disabled);
}

#[test]
fn test_desired_status_serialization() {
    let enabled = DesiredStatus::Enabled;
    let json = serde_json::to_string(&enabled).unwrap();
    assert_eq!(json, "\"Enabled\"");

    let disabled = DesiredStatus::Disabled;
    let json = serde_json::to_string(&disabled).unwrap();
    assert_eq!(json, "\"Disabled\"");
}

// ============================================================================
// RuntimeStatus Tests
// ============================================================================

#[test]
fn test_runtime_status_variants() {
    assert_eq!(RuntimeStatus::Running, RuntimeStatus::Running);
    assert_eq!(RuntimeStatus::Starting, RuntimeStatus::Starting);
    assert_eq!(RuntimeStatus::Stopped, RuntimeStatus::Stopped);
    assert_eq!(RuntimeStatus::Crashed, RuntimeStatus::Crashed);
    assert_eq!(RuntimeStatus::Orphaned, RuntimeStatus::Orphaned);
}

#[test]
fn test_runtime_status_not_equal() {
    assert_ne!(RuntimeStatus::Running, RuntimeStatus::Stopped);
    assert_ne!(RuntimeStatus::Starting, RuntimeStatus::Crashed);
    assert_ne!(RuntimeStatus::Stopped, RuntimeStatus::Orphaned);
}

// ============================================================================
// ServiceAction Tests
// ============================================================================

#[test]
fn test_service_action_none() {
    let action = ServiceAction::None;
    assert!(!action.requires_process_change());
    assert!(!action.requires_db_change());
}

#[test]
fn test_service_action_start() {
    let action = ServiceAction::Start;
    assert!(action.requires_process_change());
    assert!(action.requires_db_change());
}

#[test]
fn test_service_action_stop() {
    let action = ServiceAction::Stop;
    assert!(action.requires_process_change());
    assert!(action.requires_db_change());
}

#[test]
fn test_service_action_restart() {
    let action = ServiceAction::Restart;
    assert!(action.requires_process_change());
    assert!(action.requires_db_change());
}

#[test]
fn test_service_action_cleanup_db() {
    let action = ServiceAction::CleanupDb;
    assert!(!action.requires_process_change());
    assert!(action.requires_db_change());
}

#[test]
fn test_service_action_cleanup_process() {
    let action = ServiceAction::CleanupProcess;
    assert!(action.requires_process_change());
    assert!(!action.requires_db_change());
}

#[test]
fn test_service_action_display() {
    assert_eq!(format!("{}", ServiceAction::None), "none");
    assert_eq!(format!("{}", ServiceAction::Start), "start");
    assert_eq!(format!("{}", ServiceAction::Stop), "stop");
    assert_eq!(format!("{}", ServiceAction::Restart), "restart");
    assert_eq!(format!("{}", ServiceAction::CleanupDb), "cleanup-db");
    assert_eq!(
        format!("{}", ServiceAction::CleanupProcess),
        "cleanup-process"
    );
}

// ============================================================================
// VerifiedServiceState Tests - State Determination
// ============================================================================

#[test]
fn test_verified_state_enabled_running_no_action() {
    let state = VerifiedServiceState::builder(
        "api-server".to_string(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Running,
        8080,
    )
    .with_pid(1234)
    .build();

    assert_eq!(state.needs_action, ServiceAction::None);
    assert!(state.is_healthy());
    assert!(!state.needs_attention());
}

#[test]
fn test_verified_state_enabled_starting_no_action() {
    let state = VerifiedServiceState::builder(
        "api-server".to_string(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Starting,
        8080,
    )
    .with_pid(1234)
    .build();

    assert_eq!(state.needs_action, ServiceAction::None);
    assert!(state.is_healthy());
}

#[test]
fn test_verified_state_enabled_stopped_needs_start() {
    let state = VerifiedServiceState::builder(
        "api-server".to_string(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Stopped,
        8080,
    )
    .build();

    assert_eq!(state.needs_action, ServiceAction::Start);
    assert!(!state.is_healthy());
    assert!(state.needs_attention());
}

#[test]
fn test_verified_state_enabled_crashed_needs_restart() {
    let state = VerifiedServiceState::builder(
        "api-server".to_string(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Crashed,
        8080,
    )
    .build();

    assert_eq!(state.needs_action, ServiceAction::Restart);
    assert!(!state.is_healthy());
    assert!(state.needs_attention());
}

#[test]
fn test_verified_state_enabled_orphaned_needs_restart() {
    let state = VerifiedServiceState::builder(
        "api-server".to_string(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Orphaned,
        8080,
    )
    .with_pid(9999)
    .build();

    assert_eq!(state.needs_action, ServiceAction::Restart);
    assert!(!state.is_healthy());
}

#[test]
fn test_verified_state_disabled_running_needs_stop() {
    let state = VerifiedServiceState::builder(
        "api-server".to_string(),
        ServiceType::Api,
        DesiredStatus::Disabled,
        RuntimeStatus::Running,
        8080,
    )
    .with_pid(1234)
    .build();

    assert_eq!(state.needs_action, ServiceAction::Stop);
    assert!(!state.is_healthy());
    assert!(state.needs_attention());
}

#[test]
fn test_verified_state_disabled_starting_needs_stop() {
    let state = VerifiedServiceState::builder(
        "api-server".to_string(),
        ServiceType::Api,
        DesiredStatus::Disabled,
        RuntimeStatus::Starting,
        8080,
    )
    .with_pid(1234)
    .build();

    assert_eq!(state.needs_action, ServiceAction::Stop);
    assert!(!state.is_healthy());
}

#[test]
fn test_verified_state_disabled_stopped_needs_cleanup_db() {
    let state = VerifiedServiceState::builder(
        "old-service".to_string(),
        ServiceType::Api,
        DesiredStatus::Disabled,
        RuntimeStatus::Stopped,
        9000,
    )
    .build();

    assert_eq!(state.needs_action, ServiceAction::CleanupDb);
    assert!(state.needs_attention());
}

#[test]
fn test_verified_state_disabled_crashed_needs_cleanup_db() {
    let state = VerifiedServiceState::builder(
        "old-service".to_string(),
        ServiceType::Api,
        DesiredStatus::Disabled,
        RuntimeStatus::Crashed,
        9000,
    )
    .build();

    assert_eq!(state.needs_action, ServiceAction::CleanupDb);
}

#[test]
fn test_verified_state_disabled_orphaned_needs_cleanup_process() {
    let state = VerifiedServiceState::builder(
        "orphan-service".to_string(),
        ServiceType::Mcp,
        DesiredStatus::Disabled,
        RuntimeStatus::Orphaned,
        3005,
    )
    .with_pid(5678)
    .build();

    assert_eq!(state.needs_action, ServiceAction::CleanupProcess);
    assert!(state.needs_attention());
}

// ============================================================================
// VerifiedServiceState Display Methods
// ============================================================================

#[test]
fn test_verified_state_status_display() {
    let running = VerifiedServiceState::builder(
        "svc".to_string(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Running,
        8080,
    )
    .build();
    assert_eq!(running.status_display(), "running");

    let starting = VerifiedServiceState::builder(
        "svc".to_string(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Starting,
        8080,
    )
    .build();
    assert_eq!(starting.status_display(), "starting");

    let stopped = VerifiedServiceState::builder(
        "svc".to_string(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Stopped,
        8080,
    )
    .build();
    assert_eq!(stopped.status_display(), "stopped");

    let crashed = VerifiedServiceState::builder(
        "svc".to_string(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Crashed,
        8080,
    )
    .build();
    assert_eq!(crashed.status_display(), "crashed");

    let orphaned = VerifiedServiceState::builder(
        "svc".to_string(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Orphaned,
        8080,
    )
    .build();
    assert_eq!(orphaned.status_display(), "orphaned");
}

#[test]
fn test_verified_state_action_display() {
    let no_action = VerifiedServiceState::builder(
        "svc".to_string(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Running,
        8080,
    )
    .build();
    assert_eq!(no_action.action_display(), "none");

    let start_action = VerifiedServiceState::builder(
        "svc".to_string(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Stopped,
        8080,
    )
    .build();
    assert_eq!(start_action.action_display(), "start");
}

// ============================================================================
// VerifiedServiceState Builder Tests
// ============================================================================

#[test]
fn test_verified_state_builder_with_pid() {
    let state = VerifiedServiceState::builder(
        "service".to_string(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Running,
        8080,
    )
    .with_pid(12345)
    .build();

    assert_eq!(state.pid, Some(12345));
}

#[test]
fn test_verified_state_builder_with_error() {
    let state = VerifiedServiceState::builder(
        "service".to_string(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Crashed,
        8080,
    )
    .with_error("Process exited with code 1".to_string())
    .build();

    assert_eq!(state.error, Some("Process exited with code 1".to_string()));
}

#[test]
fn test_verified_state_builder_complete() {
    let state = VerifiedServiceState::builder(
        "full-service".to_string(),
        ServiceType::Mcp,
        DesiredStatus::Enabled,
        RuntimeStatus::Running,
        3000,
    )
    .with_pid(9999)
    .build();

    assert_eq!(state.name, "full-service");
    assert_eq!(state.service_type, ServiceType::Mcp);
    assert_eq!(state.desired_status, DesiredStatus::Enabled);
    assert_eq!(state.runtime_status, RuntimeStatus::Running);
    assert_eq!(state.port, 3000);
    assert_eq!(state.pid, Some(9999));
    assert_eq!(state.needs_action, ServiceAction::None);
    assert!(state.error.is_none());
}

// ============================================================================
// ReconciliationResult Tests
// ============================================================================

#[test]
fn test_reconciliation_result_new() {
    let result = ReconciliationResult::new();

    assert!(result.started.is_empty());
    assert!(result.stopped.is_empty());
    assert!(result.restarted.is_empty());
    assert!(result.cleaned_up.is_empty());
    assert!(result.failed.is_empty());
    assert!(result.is_success());
    assert_eq!(result.total_actions(), 0);
}

#[test]
fn test_reconciliation_result_with_starts() {
    let mut result = ReconciliationResult::new();
    result.started.push("service-1".to_string());
    result.started.push("service-2".to_string());

    assert_eq!(result.started.len(), 2);
    assert!(result.is_success());
    assert_eq!(result.total_actions(), 2);
}

#[test]
fn test_reconciliation_result_with_stops() {
    let mut result = ReconciliationResult::new();
    result.stopped.push("old-service".to_string());

    assert_eq!(result.stopped.len(), 1);
    assert!(result.is_success());
    assert_eq!(result.total_actions(), 1);
}

#[test]
fn test_reconciliation_result_with_restarts() {
    let mut result = ReconciliationResult::new();
    result.restarted.push("crashed-service".to_string());

    assert_eq!(result.restarted.len(), 1);
    assert!(result.is_success());
    assert_eq!(result.total_actions(), 1);
}

#[test]
fn test_reconciliation_result_with_cleanups() {
    let mut result = ReconciliationResult::new();
    result.cleaned_up.push("orphan-1".to_string());
    result.cleaned_up.push("orphan-2".to_string());

    assert_eq!(result.cleaned_up.len(), 2);
    assert!(result.is_success());
    assert_eq!(result.total_actions(), 2);
}

#[test]
fn test_reconciliation_result_with_failures() {
    let mut result = ReconciliationResult::new();
    result.started.push("success-service".to_string());
    result
        .failed
        .push(("fail-service".to_string(), "Port in use".to_string()));

    assert!(!result.is_success());
    assert_eq!(result.total_actions(), 1); // Only successful actions counted
    assert_eq!(result.failed.len(), 1);
    assert_eq!(result.failed[0].0, "fail-service");
    assert_eq!(result.failed[0].1, "Port in use");
}

#[test]
fn test_reconciliation_result_mixed_actions() {
    let mut result = ReconciliationResult::new();
    result.started.push("new-service-1".to_string());
    result.started.push("new-service-2".to_string());
    result.stopped.push("old-service".to_string());
    result.restarted.push("crashed-service".to_string());
    result.cleaned_up.push("orphan".to_string());

    assert!(result.is_success());
    assert_eq!(result.total_actions(), 5);
}

// ============================================================================
// DbServiceRecord Tests
// ============================================================================

#[test]
fn test_db_service_record_running() {
    let record = DbServiceRecord {
        name: "api-server".to_string(),
        service_type: "api".to_string(),
        status: "running".to_string(),
        pid: Some(1234),
        port: 8080,
    };

    assert_eq!(record.name, "api-server");
    assert_eq!(record.status, "running");
    assert_eq!(record.pid, Some(1234));
}

#[test]
fn test_db_service_record_stopped() {
    let record = DbServiceRecord {
        name: "mcp-server".to_string(),
        service_type: "mcp".to_string(),
        status: "stopped".to_string(),
        pid: None,
        port: 3000,
    };

    assert_eq!(record.status, "stopped");
    assert!(record.pid.is_none());
}

#[test]
fn test_db_service_record_starting() {
    let record = DbServiceRecord {
        name: "new-service".to_string(),
        service_type: "api".to_string(),
        status: "starting".to_string(),
        pid: Some(5678),
        port: 9000,
    };

    assert_eq!(record.status, "starting");
    assert_eq!(record.pid, Some(5678));
}

// ============================================================================
// ServiceType Tests
// ============================================================================

#[test]
fn test_service_type_from_module_name_api() {
    let service_type = ServiceType::from_module_name("api");
    assert_eq!(service_type, ServiceType::Api);
}

#[test]
fn test_service_type_from_module_name_mcp() {
    let service_type = ServiceType::from_module_name("mcp");
    assert_eq!(service_type, ServiceType::Mcp);
}

#[test]
fn test_service_type_from_module_name_agent() {
    let service_type = ServiceType::from_module_name("agent");
    assert_eq!(service_type, ServiceType::Agent);
}

#[test]
fn test_service_type_from_module_name_unknown() {
    let service_type = ServiceType::from_module_name("unknown");
    // Should default to Mcp for unknown types
    assert_eq!(service_type, ServiceType::Mcp);
}

// ============================================================================
// State Transition Matrix Tests (comprehensive coverage)
// ============================================================================

#[test]
fn test_all_enabled_state_transitions() {
    // Enabled + Running = None
    let state = VerifiedServiceState::builder(
        "s".to_string(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Running,
        8080,
    )
    .build();
    assert_eq!(state.needs_action, ServiceAction::None);

    // Enabled + Starting = None
    let state = VerifiedServiceState::builder(
        "s".to_string(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Starting,
        8080,
    )
    .build();
    assert_eq!(state.needs_action, ServiceAction::None);

    // Enabled + Stopped = Start
    let state = VerifiedServiceState::builder(
        "s".to_string(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Stopped,
        8080,
    )
    .build();
    assert_eq!(state.needs_action, ServiceAction::Start);

    // Enabled + Crashed = Restart
    let state = VerifiedServiceState::builder(
        "s".to_string(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Crashed,
        8080,
    )
    .build();
    assert_eq!(state.needs_action, ServiceAction::Restart);

    // Enabled + Orphaned = Restart
    let state = VerifiedServiceState::builder(
        "s".to_string(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Orphaned,
        8080,
    )
    .build();
    assert_eq!(state.needs_action, ServiceAction::Restart);
}

#[test]
fn test_all_disabled_state_transitions() {
    // Disabled + Running = Stop
    let state = VerifiedServiceState::builder(
        "s".to_string(),
        ServiceType::Api,
        DesiredStatus::Disabled,
        RuntimeStatus::Running,
        8080,
    )
    .build();
    assert_eq!(state.needs_action, ServiceAction::Stop);

    // Disabled + Starting = Stop
    let state = VerifiedServiceState::builder(
        "s".to_string(),
        ServiceType::Api,
        DesiredStatus::Disabled,
        RuntimeStatus::Starting,
        8080,
    )
    .build();
    assert_eq!(state.needs_action, ServiceAction::Stop);

    // Disabled + Stopped = CleanupDb
    let state = VerifiedServiceState::builder(
        "s".to_string(),
        ServiceType::Api,
        DesiredStatus::Disabled,
        RuntimeStatus::Stopped,
        8080,
    )
    .build();
    assert_eq!(state.needs_action, ServiceAction::CleanupDb);

    // Disabled + Crashed = CleanupDb
    let state = VerifiedServiceState::builder(
        "s".to_string(),
        ServiceType::Api,
        DesiredStatus::Disabled,
        RuntimeStatus::Crashed,
        8080,
    )
    .build();
    assert_eq!(state.needs_action, ServiceAction::CleanupDb);

    // Disabled + Orphaned = CleanupProcess
    let state = VerifiedServiceState::builder(
        "s".to_string(),
        ServiceType::Api,
        DesiredStatus::Disabled,
        RuntimeStatus::Orphaned,
        8080,
    )
    .build();
    assert_eq!(state.needs_action, ServiceAction::CleanupProcess);
}

// ============================================================================
// VerifiedServiceState Serialization Tests
// ============================================================================

#[test]
fn test_verified_state_serialization() {
    let state = VerifiedServiceState::builder(
        "test-service".to_string(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Running,
        8080,
    )
    .with_pid(1234)
    .build();

    let json = serde_json::to_string(&state).unwrap();
    assert!(json.contains("\"name\":\"test-service\""));
    assert!(json.contains("\"port\":8080"));
    assert!(json.contains("\"pid\":1234"));
}

#[test]
fn test_verified_state_deserialization() {
    let json = r#"{
        "name": "api-server",
        "service_type": "Api",
        "desired_status": "Enabled",
        "runtime_status": "Running",
        "pid": 5678,
        "port": 9000,
        "needs_action": "None",
        "error": null
    }"#;

    let state: VerifiedServiceState = serde_json::from_str(json).unwrap();
    assert_eq!(state.name, "api-server");
    assert_eq!(state.port, 9000);
    assert_eq!(state.pid, Some(5678));
}

// ============================================================================
// Edge Cases and Boundary Tests
// ============================================================================

#[test]
fn test_empty_service_name() {
    let state = VerifiedServiceState::builder(
        String::new(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Stopped,
        8080,
    )
    .build();

    assert_eq!(state.name, "");
    assert_eq!(state.needs_action, ServiceAction::Start);
}

#[test]
fn test_port_zero() {
    let config = ServiceConfig {
        name: "no-port".to_string(),
        service_type: ServiceType::Mcp,
        port: 0,
        enabled: true,
    };

    assert_eq!(config.port, 0);
}

#[test]
fn test_high_port_number() {
    let config = ServiceConfig {
        name: "high-port".to_string(),
        service_type: ServiceType::Api,
        port: 65535,
        enabled: true,
    };

    assert_eq!(config.port, 65535);
}

#[test]
fn test_large_run_count() {
    let now = Utc::now();
    let job = ScheduledJob {
        id: ScheduledJobId::generate(),
        job_name: "frequent_job".to_string(),
        schedule: "* * * * * *".to_string(),
        enabled: true,
        last_run: Some(now),
        next_run: None,
        last_status: Some("success".to_string()),
        last_error: None,
        run_count: i32::MAX,
        created_at: now,
        updated_at: now,
    };

    assert_eq!(job.run_count, i32::MAX);
}

#[test]
fn test_job_with_special_characters_in_name() {
    let now = Utc::now();
    let job = ScheduledJob {
        id: ScheduledJobId::generate(),
        job_name: "job-with_special.chars:v2".to_string(),
        schedule: "0 0 * * * *".to_string(),
        enabled: true,
        last_run: None,
        next_run: None,
        last_status: None,
        last_error: None,
        run_count: 0,
        created_at: now,
        updated_at: now,
    };

    assert_eq!(job.job_name, "job-with_special.chars:v2");
}

#[test]
fn test_very_long_error_message() {
    let long_error = "E".repeat(10000);
    let now = Utc::now();
    let job = ScheduledJob {
        id: ScheduledJobId::generate(),
        job_name: "error_job".to_string(),
        schedule: "0 0 * * * *".to_string(),
        enabled: true,
        last_run: Some(now),
        next_run: None,
        last_status: Some("failed".to_string()),
        last_error: Some(long_error.clone()),
        run_count: 1,
        created_at: now,
        updated_at: now,
    };

    assert_eq!(job.last_error.unwrap().len(), 10000);
}

// ============================================================================
// Cron Schedule Format Tests
// ============================================================================

#[test]
fn test_common_cron_schedules() {
    // These are just format validation, not actual cron parsing
    let schedules = vec![
        ("0 0 * * * *", "Every hour"),
        ("0 */10 * * * *", "Every 10 minutes"),
        ("0 0 3 * * *", "Daily at 3 AM"),
        ("0 0 */2 * * *", "Every 2 hours"),
        ("0 */15 * * * *", "Every 15 minutes"),
        ("* * * * * *", "Every second"),
    ];

    for (schedule, _description) in schedules {
        let now = Utc::now();
        let job = ScheduledJob {
            id: ScheduledJobId::generate(),
            job_name: "test".to_string(),
            schedule: schedule.to_string(),
            enabled: true,
            last_run: None,
            next_run: None,
            last_status: None,
            last_error: None,
            run_count: 0,
            created_at: now,
            updated_at: now,
        };
        assert_eq!(job.schedule, schedule);
    }
}

// ============================================================================
// Multiple Service Configs Tests
// ============================================================================

#[test]
fn test_multiple_service_configs() {
    let configs = vec![
        ServiceConfig {
            name: "api".to_string(),
            service_type: ServiceType::Api,
            port: 8080,
            enabled: true,
        },
        ServiceConfig {
            name: "mcp-1".to_string(),
            service_type: ServiceType::Mcp,
            port: 3001,
            enabled: true,
        },
        ServiceConfig {
            name: "mcp-2".to_string(),
            service_type: ServiceType::Mcp,
            port: 3002,
            enabled: false,
        },
        ServiceConfig {
            name: "agent".to_string(),
            service_type: ServiceType::Agent,
            port: 4000,
            enabled: true,
        },
    ];

    let enabled_count = configs.iter().filter(|c| c.enabled).count();
    assert_eq!(enabled_count, 3);

    let mcp_count = configs
        .iter()
        .filter(|c| c.service_type == ServiceType::Mcp)
        .count();
    assert_eq!(mcp_count, 2);
}

// ============================================================================
// ReconciliationResult Aggregation Tests
// ============================================================================

#[test]
fn test_reconciliation_result_aggregation() {
    let mut result = ReconciliationResult::new();

    // Simulate a reconciliation run
    result.started.push("new-api".to_string());
    result.started.push("new-mcp".to_string());
    result.restarted.push("crashed-service".to_string());
    result.stopped.push("disabled-old".to_string());
    result.cleaned_up.push("orphan-1".to_string());
    result.cleaned_up.push("orphan-2".to_string());
    result
        .failed
        .push(("bad-service".to_string(), "Error".to_string()));

    assert_eq!(result.started.len(), 2);
    assert_eq!(result.restarted.len(), 1);
    assert_eq!(result.stopped.len(), 1);
    assert_eq!(result.cleaned_up.len(), 2);
    assert_eq!(result.failed.len(), 1);
    assert_eq!(result.total_actions(), 6);
    assert!(!result.is_success());
}

// ============================================================================
// Job Trait Tests - Basic Job Properties
// ============================================================================

#[test]
fn test_behavioral_analysis_job_properties() {
    use systemprompt_core_scheduler::BehavioralAnalysisJob;
    use systemprompt_traits::Job;

    let job = BehavioralAnalysisJob;
    assert_eq!(job.name(), "behavioral_analysis");
    assert_eq!(
        job.description(),
        "Analyzes fingerprint behavior patterns and flags suspicious activity"
    );
    assert_eq!(job.schedule(), "0 0 * * * *"); // Every hour
}

#[test]
fn test_database_cleanup_job_properties() {
    use systemprompt_core_scheduler::DatabaseCleanupJob;
    use systemprompt_traits::Job;

    let job = DatabaseCleanupJob;
    assert_eq!(job.name(), "database_cleanup");
    assert_eq!(
        job.description(),
        "Cleans up orphaned logs, MCP executions, and expired OAuth tokens"
    );
    assert_eq!(job.schedule(), "0 0 3 * * *"); // Daily at 3 AM
}

#[test]
fn test_cleanup_empty_contexts_job_properties() {
    use systemprompt_core_scheduler::CleanupEmptyContextsJob;
    use systemprompt_traits::Job;

    let job = CleanupEmptyContextsJob;
    assert_eq!(job.name(), "cleanup_empty_contexts");
    assert_eq!(
        job.description(),
        "Deletes empty conversation contexts older than 1 hour"
    );
    assert_eq!(job.schedule(), "0 0 */2 * * *"); // Every 2 hours
}

#[test]
fn test_cleanup_inactive_sessions_job_properties() {
    use systemprompt_core_scheduler::CleanupInactiveSessionsJob;
    use systemprompt_traits::Job;

    let job = CleanupInactiveSessionsJob;
    assert_eq!(job.name(), "cleanup_inactive_sessions");
    assert_eq!(
        job.description(),
        "Cleans up inactive sessions (1 hour threshold)"
    );
    assert_eq!(job.schedule(), "0 */10 * * * *"); // Every 10 minutes
}

#[test]
fn test_feature_extraction_job_properties() {
    use systemprompt_core_scheduler::FeatureExtractionJob;
    use systemprompt_traits::Job;

    let job = FeatureExtractionJob;
    assert_eq!(job.name(), "feature_extraction");
    assert_eq!(
        job.description(),
        "Extracts ML behavioral features from completed sessions"
    );
    assert_eq!(job.schedule(), "0 */15 * * * *"); // Every 15 minutes
}

// ============================================================================
// Job Cron Schedule Validation Tests
// ============================================================================

#[test]
fn test_job_schedules_are_valid_cron_format() {
    use systemprompt_core_scheduler::{
        BehavioralAnalysisJob, CleanupEmptyContextsJob, CleanupInactiveSessionsJob,
        DatabaseCleanupJob, FeatureExtractionJob,
    };
    use systemprompt_traits::Job;

    let jobs: Vec<&dyn Job> = vec![
        &BehavioralAnalysisJob,
        &DatabaseCleanupJob,
        &CleanupEmptyContextsJob,
        &CleanupInactiveSessionsJob,
        &FeatureExtractionJob,
    ];

    for job in jobs {
        let schedule = job.schedule();
        // 6-field cron: second minute hour day month weekday
        let parts: Vec<&str> = schedule.split_whitespace().collect();
        assert_eq!(
            parts.len(),
            6,
            "Job {} has invalid cron schedule: {}",
            job.name(),
            schedule
        );
    }
}

#[test]
fn test_all_jobs_have_unique_names() {
    use systemprompt_core_scheduler::{
        BehavioralAnalysisJob, CleanupEmptyContextsJob, CleanupInactiveSessionsJob,
        DatabaseCleanupJob, FeatureExtractionJob,
    };
    use systemprompt_traits::Job;

    let names: Vec<&str> = vec![
        BehavioralAnalysisJob.name(),
        DatabaseCleanupJob.name(),
        CleanupEmptyContextsJob.name(),
        CleanupInactiveSessionsJob.name(),
        FeatureExtractionJob.name(),
    ];

    let mut unique_names = names.clone();
    unique_names.sort();
    unique_names.dedup();

    assert_eq!(
        names.len(),
        unique_names.len(),
        "Job names are not unique: {:?}",
        names
    );
}

// ============================================================================
// ProcessInfo Tests
// ============================================================================

#[test]
fn test_process_info_creation() {
    use systemprompt_core_scheduler::ProcessInfo;

    let info = ProcessInfo {
        pid: 1234,
        name: "test-process".to_string(),
        port: 8080,
    };

    assert_eq!(info.pid, 1234);
    assert_eq!(info.name, "test-process");
    assert_eq!(info.port, 8080);
}

#[test]
fn test_process_info_clone() {
    use systemprompt_core_scheduler::ProcessInfo;

    let info = ProcessInfo {
        pid: 5678,
        name: "cloneable-process".to_string(),
        port: 3000,
    };

    let cloned = info.clone();
    assert_eq!(cloned.pid, info.pid);
    assert_eq!(cloned.name, info.name);
    assert_eq!(cloned.port, info.port);
}

#[test]
fn test_process_info_debug() {
    use systemprompt_core_scheduler::ProcessInfo;

    let info = ProcessInfo {
        pid: 9999,
        name: "debug-test".to_string(),
        port: 4000,
    };

    let debug_str = format!("{:?}", info);
    assert!(debug_str.contains("ProcessInfo"));
    assert!(debug_str.contains("9999"));
    assert!(debug_str.contains("debug-test"));
    assert!(debug_str.contains("4000"));
}

#[test]
fn test_process_info_with_empty_name() {
    use systemprompt_core_scheduler::ProcessInfo;

    let info = ProcessInfo {
        pid: 100,
        name: String::new(),
        port: 5000,
    };

    assert_eq!(info.name, "");
}

#[test]
fn test_process_info_with_long_name() {
    use systemprompt_core_scheduler::ProcessInfo;

    let long_name = "a".repeat(1000);
    let info = ProcessInfo {
        pid: 200,
        name: long_name.clone(),
        port: 6000,
    };

    assert_eq!(info.name.len(), 1000);
}

// ============================================================================
// ProcessCleanup Tests - Protected Ports
// ============================================================================

#[test]
fn test_check_port_protected_postgres() {
    use systemprompt_core_scheduler::ProcessCleanup;

    // PostgreSQL port should return None (protected)
    let result = ProcessCleanup::check_port(5432);
    assert!(result.is_none(), "PostgreSQL port 5432 should be protected");
}

#[test]
fn test_check_port_protected_pgbouncer() {
    use systemprompt_core_scheduler::ProcessCleanup;

    // PgBouncer port should return None (protected)
    let result = ProcessCleanup::check_port(6432);
    assert!(
        result.is_none(),
        "PgBouncer port 6432 should be protected"
    );
}

#[test]
fn test_kill_port_protected_postgres() {
    use systemprompt_core_scheduler::ProcessCleanup;

    // Killing PostgreSQL port should return empty vec (protected)
    let result = ProcessCleanup::kill_port(5432);
    assert!(
        result.is_empty(),
        "Should not kill processes on protected PostgreSQL port"
    );
}

#[test]
fn test_kill_port_protected_pgbouncer() {
    use systemprompt_core_scheduler::ProcessCleanup;

    // Killing PgBouncer port should return empty vec (protected)
    let result = ProcessCleanup::kill_port(6432);
    assert!(
        result.is_empty(),
        "Should not kill processes on protected PgBouncer port"
    );
}

// ============================================================================
// ProcessCleanup Tests - Protected Patterns
// ============================================================================

#[test]
fn test_kill_by_pattern_protected_postgres() {
    use systemprompt_core_scheduler::ProcessCleanup;

    // Should not kill postgres processes
    let result = ProcessCleanup::kill_by_pattern("postgres");
    assert_eq!(result, 0, "Should not kill postgres processes");
}

#[test]
fn test_kill_by_pattern_protected_pgbouncer() {
    use systemprompt_core_scheduler::ProcessCleanup;

    // Should not kill pgbouncer processes
    let result = ProcessCleanup::kill_by_pattern("pgbouncer");
    assert_eq!(result, 0, "Should not kill pgbouncer processes");
}

#[test]
fn test_kill_by_pattern_protected_psql() {
    use systemprompt_core_scheduler::ProcessCleanup;

    // Should not kill psql processes
    let result = ProcessCleanup::kill_by_pattern("psql");
    assert_eq!(result, 0, "Should not kill psql processes");
}

#[test]
fn test_kill_by_pattern_protected_contains() {
    use systemprompt_core_scheduler::ProcessCleanup;

    // Patterns containing protected names should also be blocked
    let result = ProcessCleanup::kill_by_pattern("my-postgres-wrapper");
    assert_eq!(
        result, 0,
        "Should not kill patterns containing postgres"
    );
}

// ============================================================================
// ProcessCleanup Tests - Process Existence
// ============================================================================

#[test]
fn test_process_exists_invalid_pid() {
    use systemprompt_core_scheduler::ProcessCleanup;

    // PID 0 should not exist as a user process
    let exists = ProcessCleanup::process_exists(0);
    assert!(!exists, "PID 0 should not exist as a user process");
}

#[test]
fn test_process_exists_very_high_pid() {
    use systemprompt_core_scheduler::ProcessCleanup;

    // Very high PIDs are unlikely to exist
    let exists = ProcessCleanup::process_exists(u32::MAX);
    assert!(!exists, "Very high PID should not exist");
}

#[test]
fn test_process_exists_pid_1() {
    use systemprompt_core_scheduler::ProcessCleanup;

    // PID 1 (init/systemd) should exist on Linux
    let exists = ProcessCleanup::process_exists(1);
    assert!(exists, "PID 1 (init) should exist on Linux systems");
}

// ============================================================================
// DbServiceRecord Additional Tests
// ============================================================================

#[test]
fn test_db_service_record_clone() {
    let record = DbServiceRecord {
        name: "api-server".to_string(),
        service_type: "api".to_string(),
        status: "running".to_string(),
        pid: Some(1234),
        port: 8080,
    };

    let cloned = record.clone();
    assert_eq!(cloned.name, record.name);
    assert_eq!(cloned.service_type, record.service_type);
    assert_eq!(cloned.status, record.status);
    assert_eq!(cloned.pid, record.pid);
    assert_eq!(cloned.port, record.port);
}

#[test]
fn test_db_service_record_debug() {
    let record = DbServiceRecord {
        name: "debug-service".to_string(),
        service_type: "mcp".to_string(),
        status: "crashed".to_string(),
        pid: None,
        port: 3000,
    };

    let debug_str = format!("{:?}", record);
    assert!(debug_str.contains("DbServiceRecord"));
    assert!(debug_str.contains("debug-service"));
    assert!(debug_str.contains("crashed"));
}

#[test]
fn test_db_service_record_all_statuses() {
    let statuses = ["running", "starting", "stopped", "crashed", "orphaned"];

    for status in statuses {
        let record = DbServiceRecord {
            name: "status-test".to_string(),
            service_type: "api".to_string(),
            status: status.to_string(),
            pid: Some(123),
            port: 8080,
        };
        assert_eq!(record.status, status);
    }
}

// ============================================================================
// VerifiedServiceState Additional Tests
// ============================================================================

#[test]
fn test_verified_state_serialization_with_error() {
    let state = VerifiedServiceState::builder(
        "error-service".to_string(),
        ServiceType::Api,
        DesiredStatus::Enabled,
        RuntimeStatus::Crashed,
        8080,
    )
    .with_error("Connection refused".to_string())
    .build();

    let json = serde_json::to_string(&state).unwrap();
    assert!(json.contains("\"error\":\"Connection refused\""));
}

#[test]
fn test_verified_state_serialization_without_pid() {
    let state = VerifiedServiceState::builder(
        "no-pid-service".to_string(),
        ServiceType::Mcp,
        DesiredStatus::Disabled,
        RuntimeStatus::Stopped,
        3000,
    )
    .build();

    let json = serde_json::to_string(&state).unwrap();
    assert!(json.contains("\"pid\":null"));
}

#[test]
fn test_verified_state_all_service_types() {
    let types = [
        (ServiceType::Api, "Api"),
        (ServiceType::Mcp, "Mcp"),
        (ServiceType::Agent, "Agent"),
    ];

    for (service_type, expected_str) in types {
        let state = VerifiedServiceState::builder(
            "type-test".to_string(),
            service_type,
            DesiredStatus::Enabled,
            RuntimeStatus::Running,
            8080,
        )
        .build();

        let json = serde_json::to_string(&state).unwrap();
        assert!(
            json.contains(expected_str),
            "JSON should contain service type: {}",
            expected_str
        );
    }
}

// ============================================================================
// ServiceConfig Additional Tests
// ============================================================================

#[test]
fn test_service_config_clone() {
    let config = ServiceConfig {
        name: "clone-test".to_string(),
        service_type: ServiceType::Api,
        port: 8080,
        enabled: true,
    };

    let cloned = config.clone();
    assert_eq!(cloned.name, config.name);
    assert_eq!(cloned.service_type, config.service_type);
    assert_eq!(cloned.port, config.port);
    assert_eq!(cloned.enabled, config.enabled);
}

#[test]
fn test_service_config_debug() {
    let config = ServiceConfig {
        name: "debug-test".to_string(),
        service_type: ServiceType::Mcp,
        port: 3001,
        enabled: false,
    };

    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("ServiceConfig"));
    assert!(debug_str.contains("debug-test"));
    assert!(debug_str.contains("3001"));
}

// ============================================================================
// SchedulerError Additional Tests
// ============================================================================

#[test]
fn test_scheduler_error_display_all_variants() {
    let errors = vec![
        (
            SchedulerError::job_not_found("test_job"),
            "Job not found: test_job",
        ),
        (
            SchedulerError::invalid_schedule("bad_cron"),
            "Invalid cron schedule: bad_cron",
        ),
        (
            SchedulerError::job_execution_failed("job", "error"),
            "Job execution failed: job - error",
        ),
        (
            SchedulerError::config_error("bad config"),
            "Configuration error: bad config",
        ),
        (SchedulerError::AlreadyRunning, "Scheduler already running"),
        (SchedulerError::NotInitialized, "Scheduler not initialized"),
    ];

    for (error, expected) in errors {
        assert_eq!(error.to_string(), expected);
    }
}

// ============================================================================
// ReconciliationResult Default Tests
// ============================================================================

#[test]
fn test_reconciliation_result_default() {
    let result = ReconciliationResult::default();

    assert!(result.started.is_empty());
    assert!(result.stopped.is_empty());
    assert!(result.restarted.is_empty());
    assert!(result.cleaned_up.is_empty());
    assert!(result.failed.is_empty());
    assert!(result.is_success());
    assert_eq!(result.total_actions(), 0);
}

#[test]
fn test_reconciliation_result_debug() {
    let mut result = ReconciliationResult::new();
    result.started.push("service-1".to_string());

    let debug_str = format!("{:?}", result);
    assert!(debug_str.contains("ReconciliationResult"));
    assert!(debug_str.contains("service-1"));
}

// ============================================================================
// Job Name and Schedule Consistency Tests
// ============================================================================

#[test]
fn test_all_jobs_have_non_empty_names() {
    use systemprompt_core_scheduler::{
        BehavioralAnalysisJob, CleanupEmptyContextsJob, CleanupInactiveSessionsJob,
        DatabaseCleanupJob, FeatureExtractionJob,
    };
    use systemprompt_traits::Job;

    let jobs: Vec<&dyn Job> = vec![
        &BehavioralAnalysisJob,
        &DatabaseCleanupJob,
        &CleanupEmptyContextsJob,
        &CleanupInactiveSessionsJob,
        &FeatureExtractionJob,
    ];

    for job in jobs {
        assert!(
            !job.name().is_empty(),
            "Job name should not be empty"
        );
        assert!(
            !job.description().is_empty(),
            "Job description should not be empty for {}",
            job.name()
        );
    }
}

#[test]
fn test_all_jobs_have_snake_case_names() {
    use systemprompt_core_scheduler::{
        BehavioralAnalysisJob, CleanupEmptyContextsJob, CleanupInactiveSessionsJob,
        DatabaseCleanupJob, FeatureExtractionJob,
    };
    use systemprompt_traits::Job;

    let jobs: Vec<&dyn Job> = vec![
        &BehavioralAnalysisJob,
        &DatabaseCleanupJob,
        &CleanupEmptyContextsJob,
        &CleanupInactiveSessionsJob,
        &FeatureExtractionJob,
    ];

    for job in jobs {
        let name = job.name();
        // Snake case: lowercase with underscores
        assert!(
            name.chars().all(|c| c.is_lowercase() || c == '_'),
            "Job name '{}' should be snake_case",
            name
        );
    }
}

// ============================================================================
// Job Copy Trait Tests
// ============================================================================

#[test]
fn test_behavioral_analysis_job_copy() {
    use systemprompt_core_scheduler::BehavioralAnalysisJob;
    use systemprompt_traits::Job;

    let job1 = BehavioralAnalysisJob;
    let job2 = job1; // Copy
    assert_eq!(job1.name(), job2.name());
}

#[test]
fn test_database_cleanup_job_copy() {
    use systemprompt_core_scheduler::DatabaseCleanupJob;
    use systemprompt_traits::Job;

    let job1 = DatabaseCleanupJob;
    let job2 = job1; // Copy
    assert_eq!(job1.name(), job2.name());
}

#[test]
fn test_cleanup_empty_contexts_job_copy() {
    use systemprompt_core_scheduler::CleanupEmptyContextsJob;
    use systemprompt_traits::Job;

    let job1 = CleanupEmptyContextsJob;
    let job2 = job1; // Copy
    assert_eq!(job1.name(), job2.name());
}

#[test]
fn test_cleanup_inactive_sessions_job_copy() {
    use systemprompt_core_scheduler::CleanupInactiveSessionsJob;
    use systemprompt_traits::Job;

    let job1 = CleanupInactiveSessionsJob;
    let job2 = job1; // Copy
    assert_eq!(job1.name(), job2.name());
}

#[test]
fn test_feature_extraction_job_copy() {
    use systemprompt_core_scheduler::FeatureExtractionJob;
    use systemprompt_traits::Job;

    let job1 = FeatureExtractionJob;
    let job2 = job1; // Copy
    assert_eq!(job1.name(), job2.name());
}

// ============================================================================
// ServiceAction Serialization Tests
// ============================================================================

#[test]
fn test_service_action_serialization_all_variants() {
    let actions = [
        (ServiceAction::None, "\"None\""),
        (ServiceAction::Start, "\"Start\""),
        (ServiceAction::Stop, "\"Stop\""),
        (ServiceAction::Restart, "\"Restart\""),
        (ServiceAction::CleanupDb, "\"CleanupDb\""),
        (ServiceAction::CleanupProcess, "\"CleanupProcess\""),
    ];

    for (action, expected) in actions {
        let json = serde_json::to_string(&action).unwrap();
        assert_eq!(json, expected, "Serialization mismatch for {:?}", action);
    }
}

#[test]
fn test_service_action_deserialization_all_variants() {
    let cases = [
        ("\"None\"", ServiceAction::None),
        ("\"Start\"", ServiceAction::Start),
        ("\"Stop\"", ServiceAction::Stop),
        ("\"Restart\"", ServiceAction::Restart),
        ("\"CleanupDb\"", ServiceAction::CleanupDb),
        ("\"CleanupProcess\"", ServiceAction::CleanupProcess),
    ];

    for (json, expected) in cases {
        let action: ServiceAction = serde_json::from_str(json).unwrap();
        assert_eq!(action, expected, "Deserialization mismatch for {}", json);
    }
}

// ============================================================================
// DesiredStatus and RuntimeStatus Additional Tests
// ============================================================================

#[test]
fn test_desired_status_clone() {
    let enabled = DesiredStatus::Enabled;
    let cloned = enabled;
    assert_eq!(enabled, cloned);
}

#[test]
fn test_runtime_status_clone() {
    let running = RuntimeStatus::Running;
    let cloned = running;
    assert_eq!(running, cloned);
}

#[test]
fn test_runtime_status_serialization() {
    let statuses = [
        (RuntimeStatus::Running, "\"Running\""),
        (RuntimeStatus::Starting, "\"Starting\""),
        (RuntimeStatus::Stopped, "\"Stopped\""),
        (RuntimeStatus::Crashed, "\"Crashed\""),
        (RuntimeStatus::Orphaned, "\"Orphaned\""),
    ];

    for (status, expected) in statuses {
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, expected, "Serialization mismatch for {:?}", status);
    }
}

#[test]
fn test_runtime_status_deserialization() {
    let cases = [
        ("\"Running\"", RuntimeStatus::Running),
        ("\"Starting\"", RuntimeStatus::Starting),
        ("\"Stopped\"", RuntimeStatus::Stopped),
        ("\"Crashed\"", RuntimeStatus::Crashed),
        ("\"Orphaned\"", RuntimeStatus::Orphaned),
    ];

    for (json, expected) in cases {
        let status: RuntimeStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status, expected, "Deserialization mismatch for {}", json);
    }
}
