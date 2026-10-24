//! Tests for VerifiedServiceState

use systemprompt_identifiers::ServiceName;
use systemprompt_manifest::{ServiceStatus, ServiceType};
use systemprompt_scheduler::{DesiredStatus, ServiceAction, VerifiedService, VerifiedServiceState};

fn build_state(desired: DesiredStatus, runtime: VerifiedService) -> VerifiedServiceState {
    VerifiedServiceState::builder(
        ServiceName::new("test-service"),
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
        let state = build_state(
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Running),
        );
        assert_eq!(state.needs_action, ServiceAction::None);
    }

    #[test]
    fn enabled_starting_needs_no_action() {
        let state = build_state(
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Starting),
        );
        assert_eq!(state.needs_action, ServiceAction::None);
    }

    #[test]
    fn enabled_stopped_needs_start() {
        let state = build_state(
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Stopped),
        );
        assert_eq!(state.needs_action, ServiceAction::Start);
    }

    #[test]
    fn enabled_crashed_needs_restart() {
        let state = build_state(
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Error),
        );
        assert_eq!(state.needs_action, ServiceAction::Restart);
    }

    #[test]
    fn enabled_orphaned_needs_restart() {
        let state = build_state(DesiredStatus::Enabled, VerifiedService::orphaned());
        assert_eq!(state.needs_action, ServiceAction::Restart);
    }

    #[test]
    fn disabled_running_needs_stop() {
        let state = build_state(
            DesiredStatus::Disabled,
            VerifiedService::observed(ServiceStatus::Running),
        );
        assert_eq!(state.needs_action, ServiceAction::Stop);
    }

    #[test]
    fn disabled_starting_needs_stop() {
        let state = build_state(
            DesiredStatus::Disabled,
            VerifiedService::observed(ServiceStatus::Starting),
        );
        assert_eq!(state.needs_action, ServiceAction::Stop);
    }

    #[test]
    fn disabled_stopped_needs_cleanup_db() {
        let state = build_state(
            DesiredStatus::Disabled,
            VerifiedService::observed(ServiceStatus::Stopped),
        );
        assert_eq!(state.needs_action, ServiceAction::CleanupDb);
    }

    #[test]
    fn disabled_crashed_needs_cleanup_db() {
        let state = build_state(
            DesiredStatus::Disabled,
            VerifiedService::observed(ServiceStatus::Error),
        );
        assert_eq!(state.needs_action, ServiceAction::CleanupDb);
    }

    #[test]
    fn disabled_orphaned_needs_cleanup_process() {
        let state = build_state(DesiredStatus::Disabled, VerifiedService::orphaned());
        assert_eq!(state.needs_action, ServiceAction::CleanupProcess);
    }
}

mod health_check_tests {
    use super::*;

    #[test]
    fn enabled_running_is_healthy() {
        let state = build_state(
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Running),
        );
        assert!(state.is_healthy());
    }

    #[test]
    fn enabled_starting_is_healthy() {
        let state = build_state(
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Starting),
        );
        assert!(state.is_healthy());
    }

    #[test]
    fn enabled_stopped_is_not_healthy() {
        let state = build_state(
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Stopped),
        );
        assert!(!state.is_healthy());
    }

    #[test]
    fn enabled_crashed_is_not_healthy() {
        let state = build_state(
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Error),
        );
        assert!(!state.is_healthy());
    }

    #[test]
    fn disabled_running_is_not_healthy() {
        let state = build_state(
            DesiredStatus::Disabled,
            VerifiedService::observed(ServiceStatus::Running),
        );
        assert!(!state.is_healthy());
    }

    #[test]
    fn disabled_stopped_is_not_healthy() {
        let state = build_state(
            DesiredStatus::Disabled,
            VerifiedService::observed(ServiceStatus::Stopped),
        );
        assert!(!state.is_healthy());
    }
}

mod needs_attention_tests {
    use super::*;

    #[test]
    fn no_action_does_not_need_attention() {
        let state = build_state(
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Running),
        );
        assert!(!state.needs_attention());
    }

    #[test]
    fn start_action_needs_attention() {
        let state = build_state(
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Stopped),
        );
        assert!(state.needs_attention());
    }

    #[test]
    fn stop_action_needs_attention() {
        let state = build_state(
            DesiredStatus::Disabled,
            VerifiedService::observed(ServiceStatus::Running),
        );
        assert!(state.needs_attention());
    }

    #[test]
    fn restart_action_needs_attention() {
        let state = build_state(
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Error),
        );
        assert!(state.needs_attention());
    }

    #[test]
    fn cleanup_db_action_needs_attention() {
        let state = build_state(
            DesiredStatus::Disabled,
            VerifiedService::observed(ServiceStatus::Stopped),
        );
        assert!(state.needs_attention());
    }

    #[test]
    fn cleanup_process_action_needs_attention() {
        let state = build_state(DesiredStatus::Disabled, VerifiedService::orphaned());
        assert!(state.needs_attention());
    }
}

mod display_tests {
    use super::*;

    #[test]
    fn status_display_running() {
        let state = build_state(
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Running),
        );
        assert_eq!(state.status_display(), "running");
    }

    #[test]
    fn status_display_starting() {
        let state = build_state(
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Starting),
        );
        assert_eq!(state.status_display(), "starting");
    }

    #[test]
    fn status_display_stopped() {
        let state = build_state(
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Stopped),
        );
        assert_eq!(state.status_display(), "stopped");
    }

    #[test]
    fn status_display_error() {
        let state = build_state(
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Error),
        );
        assert_eq!(state.status_display(), "error");
    }

    #[test]
    fn status_display_orphaned() {
        let state = build_state(DesiredStatus::Enabled, VerifiedService::orphaned());
        assert_eq!(state.status_display(), "orphaned");
    }

    #[test]
    fn action_display_none() {
        let state = build_state(
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Running),
        );
        assert_eq!(state.action_display(), "-");
    }

    #[test]
    fn action_display_start() {
        let state = build_state(
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Stopped),
        );
        assert_eq!(state.action_display(), "start");
    }

    #[test]
    fn action_display_stop() {
        let state = build_state(
            DesiredStatus::Disabled,
            VerifiedService::observed(ServiceStatus::Running),
        );
        assert_eq!(state.action_display(), "stop");
    }

    #[test]
    fn action_display_restart() {
        let state = build_state(
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Error),
        );
        assert_eq!(state.action_display(), "restart");
    }

    #[test]
    fn action_display_cleanup_db() {
        let state = build_state(
            DesiredStatus::Disabled,
            VerifiedService::observed(ServiceStatus::Stopped),
        );
        assert_eq!(state.action_display(), "cleanup-db");
    }

    #[test]
    fn action_display_cleanup_process() {
        let state = build_state(DesiredStatus::Disabled, VerifiedService::orphaned());
        assert_eq!(state.action_display(), "cleanup-process");
    }
}

mod builder_tests {
    use super::*;

    #[test]
    fn builder_sets_name() {
        let state = VerifiedServiceState::builder(
            ServiceName::new("my-service"),
            ServiceType::Mcp,
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Running),
            9000,
        )
        .build();
        assert_eq!(state.name, "my-service");
    }

    #[test]
    fn builder_sets_service_type() {
        let state = VerifiedServiceState::builder(
            ServiceName::new("test"),
            ServiceType::Mcp,
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Running),
            9000,
        )
        .build();
        assert_eq!(state.service_type, ServiceType::Mcp);
    }

    #[test]
    fn builder_sets_port() {
        let state = VerifiedServiceState::builder(
            ServiceName::new("test"),
            ServiceType::Mcp,
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Running),
            3000,
        )
        .build();
        assert_eq!(state.port, 3000);
    }

    #[test]
    fn builder_with_pid() {
        let state = VerifiedServiceState::builder(
            ServiceName::new("test"),
            ServiceType::Mcp,
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Running),
            8080,
        )
        .with_pid(12345)
        .build();
        assert_eq!(state.pid, Some(12345));
    }

    #[test]
    fn builder_without_pid_is_none() {
        let state = VerifiedServiceState::builder(
            ServiceName::new("test"),
            ServiceType::Mcp,
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Running),
            8080,
        )
        .build();
        assert_eq!(state.pid, None);
    }

    #[test]
    fn builder_with_error() {
        let state = VerifiedServiceState::builder(
            ServiceName::new("test"),
            ServiceType::Mcp,
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Error),
            8080,
        )
        .with_error("Connection refused".to_string())
        .build();
        assert_eq!(state.error, Some("Connection refused".to_string()));
    }

    #[test]
    fn builder_without_error_is_none() {
        let state = VerifiedServiceState::builder(
            ServiceName::new("test"),
            ServiceType::Mcp,
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Running),
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
        let state = build_state(
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Running),
        );
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("test-service"));
        assert!(json.contains("8080"));
    }

    #[test]
    fn state_is_debug() {
        let state = build_state(
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Running),
        );
        let debug = format!("{:?}", state);
        assert!(debug.contains("test-service"));
    }
}

mod verified_service_tests {
    use super::*;

    #[test]
    fn observed_running_and_starting_are_healthy() {
        assert!(VerifiedService::observed(ServiceStatus::Running).is_healthy());
        assert!(VerifiedService::observed(ServiceStatus::Starting).is_healthy());
        assert!(!VerifiedService::observed(ServiceStatus::Stopped).is_healthy());
        assert!(!VerifiedService::observed(ServiceStatus::Error).is_healthy());
    }

    #[test]
    fn orphaned_process_is_never_healthy_and_needs_cleanup() {
        let orphaned = VerifiedService::orphaned();
        assert!(orphaned.orphaned);
        assert!(!orphaned.is_healthy());
        assert!(orphaned.needs_cleanup());
        assert!(VerifiedService::observed(ServiceStatus::Error).needs_cleanup());
        assert!(!VerifiedService::observed(ServiceStatus::Running).needs_cleanup());
    }

    #[test]
    fn orphaned_wins_over_status_in_the_display() {
        let state = build_state(DesiredStatus::Disabled, VerifiedService::orphaned());
        assert_eq!(state.status_display(), "orphaned");
        assert_eq!(state.needs_action, ServiceAction::CleanupProcess);
    }

    #[test]
    fn enabled_stopping_row_is_left_alone() {
        let state = build_state(
            DesiredStatus::Enabled,
            VerifiedService::observed(ServiceStatus::Stopping),
        );
        assert_eq!(state.needs_action, ServiceAction::None);
        assert_eq!(state.status_display(), "stopping");
    }
}
