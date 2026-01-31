//! Tests for VerifiedServiceState

use systemprompt_models::{RuntimeStatus, ServiceType};
use systemprompt_scheduler::{DesiredStatus, ServiceAction, VerifiedServiceState};

fn build_state(
    desired: DesiredStatus,
    runtime: RuntimeStatus,
) -> VerifiedServiceState {
    VerifiedServiceState::builder(
        "test-service".to_string(),
        ServiceType::Mcp,
        desired,
        runtime,
        8080,
    )
    .build()
}

mod action_determination_tests {
    use super::*;

    #[test]
    fn enabled_running_needs_no_action() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Running);
        assert_eq!(state.needs_action, ServiceAction::None);
    }

    #[test]
    fn enabled_starting_needs_no_action() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Starting);
        assert_eq!(state.needs_action, ServiceAction::None);
    }

    #[test]
    fn enabled_stopped_needs_start() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Stopped);
        assert_eq!(state.needs_action, ServiceAction::Start);
    }

    #[test]
    fn enabled_crashed_needs_restart() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Crashed);
        assert_eq!(state.needs_action, ServiceAction::Restart);
    }

    #[test]
    fn enabled_orphaned_needs_restart() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Orphaned);
        assert_eq!(state.needs_action, ServiceAction::Restart);
    }

    #[test]
    fn disabled_running_needs_stop() {
        let state = build_state(DesiredStatus::Disabled, RuntimeStatus::Running);
        assert_eq!(state.needs_action, ServiceAction::Stop);
    }

    #[test]
    fn disabled_starting_needs_stop() {
        let state = build_state(DesiredStatus::Disabled, RuntimeStatus::Starting);
        assert_eq!(state.needs_action, ServiceAction::Stop);
    }

    #[test]
    fn disabled_stopped_needs_cleanup_db() {
        let state = build_state(DesiredStatus::Disabled, RuntimeStatus::Stopped);
        assert_eq!(state.needs_action, ServiceAction::CleanupDb);
    }

    #[test]
    fn disabled_crashed_needs_cleanup_db() {
        let state = build_state(DesiredStatus::Disabled, RuntimeStatus::Crashed);
        assert_eq!(state.needs_action, ServiceAction::CleanupDb);
    }

    #[test]
    fn disabled_orphaned_needs_cleanup_process() {
        let state = build_state(DesiredStatus::Disabled, RuntimeStatus::Orphaned);
        assert_eq!(state.needs_action, ServiceAction::CleanupProcess);
    }
}

mod health_check_tests {
    use super::*;

    #[test]
    fn enabled_running_is_healthy() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Running);
        assert!(state.is_healthy());
    }

    #[test]
    fn enabled_starting_is_healthy() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Starting);
        assert!(state.is_healthy());
    }

    #[test]
    fn enabled_stopped_is_not_healthy() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Stopped);
        assert!(!state.is_healthy());
    }

    #[test]
    fn enabled_crashed_is_not_healthy() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Crashed);
        assert!(!state.is_healthy());
    }

    #[test]
    fn disabled_running_is_not_healthy() {
        let state = build_state(DesiredStatus::Disabled, RuntimeStatus::Running);
        assert!(!state.is_healthy());
    }

    #[test]
    fn disabled_stopped_is_not_healthy() {
        let state = build_state(DesiredStatus::Disabled, RuntimeStatus::Stopped);
        assert!(!state.is_healthy());
    }
}

mod needs_attention_tests {
    use super::*;

    #[test]
    fn no_action_does_not_need_attention() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Running);
        assert!(!state.needs_attention());
    }

    #[test]
    fn start_action_needs_attention() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Stopped);
        assert!(state.needs_attention());
    }

    #[test]
    fn stop_action_needs_attention() {
        let state = build_state(DesiredStatus::Disabled, RuntimeStatus::Running);
        assert!(state.needs_attention());
    }

    #[test]
    fn restart_action_needs_attention() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Crashed);
        assert!(state.needs_attention());
    }

    #[test]
    fn cleanup_db_action_needs_attention() {
        let state = build_state(DesiredStatus::Disabled, RuntimeStatus::Stopped);
        assert!(state.needs_attention());
    }

    #[test]
    fn cleanup_process_action_needs_attention() {
        let state = build_state(DesiredStatus::Disabled, RuntimeStatus::Orphaned);
        assert!(state.needs_attention());
    }
}

mod display_tests {
    use super::*;

    #[test]
    fn status_display_running() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Running);
        assert_eq!(state.status_display(), "running");
    }

    #[test]
    fn status_display_starting() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Starting);
        assert_eq!(state.status_display(), "starting");
    }

    #[test]
    fn status_display_stopped() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Stopped);
        assert_eq!(state.status_display(), "stopped");
    }

    #[test]
    fn status_display_crashed() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Crashed);
        assert_eq!(state.status_display(), "crashed");
    }

    #[test]
    fn status_display_orphaned() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Orphaned);
        assert_eq!(state.status_display(), "orphaned");
    }

    #[test]
    fn action_display_none() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Running);
        assert_eq!(state.action_display(), "-");
    }

    #[test]
    fn action_display_start() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Stopped);
        assert_eq!(state.action_display(), "start");
    }

    #[test]
    fn action_display_stop() {
        let state = build_state(DesiredStatus::Disabled, RuntimeStatus::Running);
        assert_eq!(state.action_display(), "stop");
    }

    #[test]
    fn action_display_restart() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Crashed);
        assert_eq!(state.action_display(), "restart");
    }

    #[test]
    fn action_display_cleanup_db() {
        let state = build_state(DesiredStatus::Disabled, RuntimeStatus::Stopped);
        assert_eq!(state.action_display(), "cleanup-db");
    }

    #[test]
    fn action_display_cleanup_process() {
        let state = build_state(DesiredStatus::Disabled, RuntimeStatus::Orphaned);
        assert_eq!(state.action_display(), "cleanup-process");
    }
}

mod builder_tests {
    use super::*;

    #[test]
    fn builder_sets_name() {
        let state = VerifiedServiceState::builder(
            "my-service".to_string(),
            ServiceType::Mcp,
            DesiredStatus::Enabled,
            RuntimeStatus::Running,
            9000,
        )
        .build();
        assert_eq!(state.name, "my-service");
    }

    #[test]
    fn builder_sets_service_type() {
        let state = VerifiedServiceState::builder(
            "test".to_string(),
            ServiceType::Mcp,
            DesiredStatus::Enabled,
            RuntimeStatus::Running,
            9000,
        )
        .build();
        assert_eq!(state.service_type, ServiceType::Mcp);
    }

    #[test]
    fn builder_sets_port() {
        let state = VerifiedServiceState::builder(
            "test".to_string(),
            ServiceType::Mcp,
            DesiredStatus::Enabled,
            RuntimeStatus::Running,
            3000,
        )
        .build();
        assert_eq!(state.port, 3000);
    }

    #[test]
    fn builder_with_pid() {
        let state = VerifiedServiceState::builder(
            "test".to_string(),
            ServiceType::Mcp,
            DesiredStatus::Enabled,
            RuntimeStatus::Running,
            8080,
        )
        .with_pid(12345)
        .build();
        assert_eq!(state.pid, Some(12345));
    }

    #[test]
    fn builder_without_pid_is_none() {
        let state = VerifiedServiceState::builder(
            "test".to_string(),
            ServiceType::Mcp,
            DesiredStatus::Enabled,
            RuntimeStatus::Running,
            8080,
        )
        .build();
        assert_eq!(state.pid, None);
    }

    #[test]
    fn builder_with_error() {
        let state = VerifiedServiceState::builder(
            "test".to_string(),
            ServiceType::Mcp,
            DesiredStatus::Enabled,
            RuntimeStatus::Crashed,
            8080,
        )
        .with_error("Connection refused".to_string())
        .build();
        assert_eq!(state.error, Some("Connection refused".to_string()));
    }

    #[test]
    fn builder_without_error_is_none() {
        let state = VerifiedServiceState::builder(
            "test".to_string(),
            ServiceType::Mcp,
            DesiredStatus::Enabled,
            RuntimeStatus::Running,
            8080,
        )
        .build();
        assert_eq!(state.error, None);
    }
}

mod serialization_tests {
    use super::*;

    #[test]
    fn state_is_serializable() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Running);
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("test-service"));
        assert!(json.contains("8080"));
    }

    #[test]
    fn state_is_deserializable() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Running);
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: VerifiedServiceState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, state.name);
        assert_eq!(deserialized.port, state.port);
    }

    #[test]
    fn state_is_clone() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Running);
        let cloned = state.clone();
        assert_eq!(cloned.name, state.name);
        assert_eq!(cloned.port, state.port);
    }

    #[test]
    fn state_is_debug() {
        let state = build_state(DesiredStatus::Enabled, RuntimeStatus::Running);
        let debug = format!("{:?}", state);
        assert!(debug.contains("test-service"));
    }
}
