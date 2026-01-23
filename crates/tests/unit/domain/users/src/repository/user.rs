//! Unit tests for user repository types.
//!
//! Tests cover:
//! - MergeResult struct

use systemprompt_users::MergeResult;

// ============================================================================
// MergeResult Tests
// ============================================================================

mod merge_result_tests {
    use super::*;

    #[test]
    fn merge_result_creation() {
        let result = MergeResult {
            sessions_transferred: 5,
            tasks_transferred: 10,
        };

        assert_eq!(result.sessions_transferred, 5);
        assert_eq!(result.tasks_transferred, 10);
    }

    #[test]
    fn merge_result_with_zero_transfers() {
        let result = MergeResult {
            sessions_transferred: 0,
            tasks_transferred: 0,
        };

        assert_eq!(result.sessions_transferred, 0);
        assert_eq!(result.tasks_transferred, 0);
    }

    #[test]
    fn merge_result_with_large_numbers() {
        let result = MergeResult {
            sessions_transferred: 1_000_000,
            tasks_transferred: 500_000,
        };

        assert_eq!(result.sessions_transferred, 1_000_000);
        assert_eq!(result.tasks_transferred, 500_000);
    }

    #[test]
    fn merge_result_is_copy() {
        let result = MergeResult {
            sessions_transferred: 3,
            tasks_transferred: 7,
        };

        let copied = result;
        assert_eq!(result.sessions_transferred, copied.sessions_transferred);
        assert_eq!(result.tasks_transferred, copied.tasks_transferred);
    }

    #[test]
    fn merge_result_is_clone() {
        let result = MergeResult {
            sessions_transferred: 3,
            tasks_transferred: 7,
        };

        let cloned = result;
        assert_eq!(result.sessions_transferred, cloned.sessions_transferred);
    }

    #[test]
    fn merge_result_debug() {
        let result = MergeResult {
            sessions_transferred: 5,
            tasks_transferred: 10,
        };

        let debug = format!("{:?}", result);
        assert!(debug.contains("MergeResult"));
        assert!(debug.contains("5"));
        assert!(debug.contains("10"));
    }

    #[test]
    fn merge_result_only_sessions() {
        let result = MergeResult {
            sessions_transferred: 15,
            tasks_transferred: 0,
        };

        assert_eq!(result.sessions_transferred, 15);
        assert_eq!(result.tasks_transferred, 0);
    }

    #[test]
    fn merge_result_only_tasks() {
        let result = MergeResult {
            sessions_transferred: 0,
            tasks_transferred: 25,
        };

        assert_eq!(result.sessions_transferred, 0);
        assert_eq!(result.tasks_transferred, 25);
    }
}
