use systemprompt_scheduler::{
    DesiredStatus, ReconciliationResult, RuntimeStatus, SchedulerError, ServiceAction, ServiceType,
    VerifiedServiceState,
};

mod enabled_state_transitions {
    use super::*;

    #[test]
    fn all_enabled_state_transitions() {
        let state = VerifiedServiceState::builder(
            "s".to_string(),
            ServiceType::Api,
            DesiredStatus::Enabled,
            RuntimeStatus::Running,
            8080,
        )
        .build();
        assert_eq!(state.needs_action, ServiceAction::None);

        let state = VerifiedServiceState::builder(
            "s".to_string(),
            ServiceType::Api,
            DesiredStatus::Enabled,
            RuntimeStatus::Starting,
            8080,
        )
        .build();
        assert_eq!(state.needs_action, ServiceAction::None);

        let state = VerifiedServiceState::builder(
            "s".to_string(),
            ServiceType::Api,
            DesiredStatus::Enabled,
            RuntimeStatus::Stopped,
            8080,
        )
        .build();
        assert_eq!(state.needs_action, ServiceAction::Start);

        let state = VerifiedServiceState::builder(
            "s".to_string(),
            ServiceType::Api,
            DesiredStatus::Enabled,
            RuntimeStatus::Crashed,
            8080,
        )
        .build();
        assert_eq!(state.needs_action, ServiceAction::Restart);

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
}

mod disabled_state_transitions {
    use super::*;

    #[test]
    fn all_disabled_state_transitions() {
        let state = VerifiedServiceState::builder(
            "s".to_string(),
            ServiceType::Api,
            DesiredStatus::Disabled,
            RuntimeStatus::Running,
            8080,
        )
        .build();
        assert_eq!(state.needs_action, ServiceAction::Stop);

        let state = VerifiedServiceState::builder(
            "s".to_string(),
            ServiceType::Api,
            DesiredStatus::Disabled,
            RuntimeStatus::Starting,
            8080,
        )
        .build();
        assert_eq!(state.needs_action, ServiceAction::Stop);

        let state = VerifiedServiceState::builder(
            "s".to_string(),
            ServiceType::Api,
            DesiredStatus::Disabled,
            RuntimeStatus::Stopped,
            8080,
        )
        .build();
        assert_eq!(state.needs_action, ServiceAction::CleanupDb);

        let state = VerifiedServiceState::builder(
            "s".to_string(),
            ServiceType::Api,
            DesiredStatus::Disabled,
            RuntimeStatus::Crashed,
            8080,
        )
        .build();
        assert_eq!(state.needs_action, ServiceAction::CleanupDb);

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
}

mod verified_state_serialization_tests {
    use super::*;

    #[test]
    fn serialization() {
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
    fn deserialization() {
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

    #[test]
    fn serialization_with_error() {
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
    fn serialization_without_pid() {
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
    fn all_service_types() {
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
}

mod edge_cases {
    use super::*;

    #[test]
    fn empty_service_name() {
        let state = VerifiedServiceState::builder(
            "".to_string(),
            ServiceType::Api,
            DesiredStatus::Enabled,
            RuntimeStatus::Stopped,
            8080,
        )
        .build();

        assert_eq!(state.name, "");
        assert_eq!(state.needs_action, ServiceAction::Start);
    }
}

mod scheduler_error_additional_tests {
    use super::*;

    #[test]
    fn already_running() {
        let error = SchedulerError::AlreadyRunning;
        assert_eq!(error.to_string(), "Scheduler already running");
    }

    #[test]
    fn not_initialized() {
        let error = SchedulerError::NotInitialized;
        assert_eq!(error.to_string(), "Scheduler not initialized");
    }

    #[test]
    fn display_all_variants() {
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
}

mod reconciliation_additional_tests {
    use super::*;

    #[test]
    fn default() {
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
    fn debug() {
        let mut result = ReconciliationResult::new();
        result.started.push("service-1".to_string());

        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("ReconciliationResult"));
        assert!(debug_str.contains("service-1"));
    }

    #[test]
    fn aggregation() {
        let mut result = ReconciliationResult::new();

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
}
