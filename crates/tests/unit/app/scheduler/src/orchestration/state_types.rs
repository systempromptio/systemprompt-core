//! Tests for orchestration state types

use systemprompt_scheduler::{DesiredStatus, ServiceAction};

mod desired_status_tests {
    use super::*;

    #[test]
    fn is_debug() {
        let debug = format!("{:?}", DesiredStatus::Enabled);
        assert!(debug.contains("Enabled"));
    }
}

mod service_action_tests {
    use super::*;

    #[test]
    fn none_does_not_require_process_change() {
        assert!(!ServiceAction::None.requires_process_change());
    }

    #[test]
    fn start_requires_process_change() {
        assert!(ServiceAction::Start.requires_process_change());
    }

    #[test]
    fn stop_requires_process_change() {
        assert!(ServiceAction::Stop.requires_process_change());
    }

    #[test]
    fn restart_requires_process_change() {
        assert!(ServiceAction::Restart.requires_process_change());
    }

    #[test]
    fn cleanup_db_does_not_require_process_change() {
        assert!(!ServiceAction::CleanupDb.requires_process_change());
    }

    #[test]
    fn cleanup_process_requires_process_change() {
        assert!(ServiceAction::CleanupProcess.requires_process_change());
    }

    #[test]
    fn none_does_not_require_db_change() {
        assert!(!ServiceAction::None.requires_db_change());
    }

    #[test]
    fn start_requires_db_change() {
        assert!(ServiceAction::Start.requires_db_change());
    }

    #[test]
    fn stop_requires_db_change() {
        assert!(ServiceAction::Stop.requires_db_change());
    }

    #[test]
    fn restart_requires_db_change() {
        assert!(ServiceAction::Restart.requires_db_change());
    }

    #[test]
    fn cleanup_db_requires_db_change() {
        assert!(ServiceAction::CleanupDb.requires_db_change());
    }

    #[test]
    fn cleanup_process_does_not_require_db_change() {
        assert!(!ServiceAction::CleanupProcess.requires_db_change());
    }

    #[test]
    fn is_debug() {
        let debug = format!("{:?}", ServiceAction::Restart);
        assert!(debug.contains("Restart"));
    }
}
