//! Tests for orchestration state types

use systemprompt_scheduler::{DesiredStatus, ServiceAction};

mod desired_status_tests {
    use super::*;

    #[test]
    fn enabled_serializes_correctly() {
        let json = serde_json::to_string(&DesiredStatus::Enabled).unwrap();
        assert_eq!(json, "\"Enabled\"");
    }

    #[test]
    fn disabled_serializes_correctly() {
        let json = serde_json::to_string(&DesiredStatus::Disabled).unwrap();
        assert_eq!(json, "\"Disabled\"");
    }

    #[test]
    fn deserializes_enabled() {
        let status: DesiredStatus = serde_json::from_str("\"Enabled\"").unwrap();
        assert_eq!(status, DesiredStatus::Enabled);
    }

    #[test]
    fn deserializes_disabled() {
        let status: DesiredStatus = serde_json::from_str("\"Disabled\"").unwrap();
        assert_eq!(status, DesiredStatus::Disabled);
    }

    #[test]
    fn is_clone() {
        let status = DesiredStatus::Enabled;
        let cloned = status;
        assert_eq!(status, cloned);
    }

    #[test]
    fn is_copy() {
        let status = DesiredStatus::Disabled;
        let copied: DesiredStatus = status;
        assert_eq!(status, copied);
    }

    #[test]
    fn is_eq() {
        assert_eq!(DesiredStatus::Enabled, DesiredStatus::Enabled);
        assert_ne!(DesiredStatus::Enabled, DesiredStatus::Disabled);
    }

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
    fn serializes_none() {
        let json = serde_json::to_string(&ServiceAction::None).unwrap();
        assert_eq!(json, "\"None\"");
    }

    #[test]
    fn serializes_start() {
        let json = serde_json::to_string(&ServiceAction::Start).unwrap();
        assert_eq!(json, "\"Start\"");
    }

    #[test]
    fn serializes_stop() {
        let json = serde_json::to_string(&ServiceAction::Stop).unwrap();
        assert_eq!(json, "\"Stop\"");
    }

    #[test]
    fn serializes_restart() {
        let json = serde_json::to_string(&ServiceAction::Restart).unwrap();
        assert_eq!(json, "\"Restart\"");
    }

    #[test]
    fn serializes_cleanup_db() {
        let json = serde_json::to_string(&ServiceAction::CleanupDb).unwrap();
        assert_eq!(json, "\"CleanupDb\"");
    }

    #[test]
    fn serializes_cleanup_process() {
        let json = serde_json::to_string(&ServiceAction::CleanupProcess).unwrap();
        assert_eq!(json, "\"CleanupProcess\"");
    }

    #[test]
    fn is_clone() {
        let action = ServiceAction::Start;
        let cloned = action;
        assert_eq!(action, cloned);
    }

    #[test]
    fn is_copy() {
        let action = ServiceAction::Stop;
        let copied: ServiceAction = action;
        assert_eq!(action, copied);
    }

    #[test]
    fn is_eq() {
        assert_eq!(ServiceAction::None, ServiceAction::None);
        assert_ne!(ServiceAction::Start, ServiceAction::Stop);
    }

    #[test]
    fn is_debug() {
        let debug = format!("{:?}", ServiceAction::Restart);
        assert!(debug.contains("Restart"));
    }
}
