//! Unit tests for user repository types.
//!
//! Tests cover:
//! - MergeResult struct

use systemprompt_users::MergeResult;

mod merge_result_tests {
    use super::*;

    #[test]
    fn merge_result_creation() {
        let result = MergeResult {
            sessions: 5,
            tasks: 10,
            total_rows: 10,
        };

        assert_eq!(result.sessions, 5);
        assert_eq!(result.tasks, 10);
    }

    #[test]
    fn merge_result_with_zero_transfers() {
        let result = MergeResult {
            sessions: 0,
            tasks: 0,
            total_rows: 0,
        };

        assert_eq!(result.sessions, 0);
        assert_eq!(result.tasks, 0);
    }

    #[test]
    fn merge_result_with_large_numbers() {
        let result = MergeResult {
            sessions: 1_000_000,
            tasks: 500_000,
            total_rows: 500_000,
        };

        assert_eq!(result.sessions, 1_000_000);
        assert_eq!(result.tasks, 500_000);
    }

    #[test]
    fn merge_result_debug() {
        let result = MergeResult {
            sessions: 5,
            tasks: 10,
            total_rows: 10,
        };

        let debug = format!("{:?}", result);
        assert!(debug.contains("MergeResult"));
        assert!(debug.contains("5"));
        assert!(debug.contains("10"));
    }

    #[test]
    fn merge_result_only_sessions() {
        let result = MergeResult {
            sessions: 15,
            tasks: 0,
            total_rows: 0,
        };

        assert_eq!(result.sessions, 15);
        assert_eq!(result.tasks, 0);
    }

    #[test]
    fn merge_result_only_tasks() {
        let result = MergeResult {
            sessions: 0,
            tasks: 25,
            total_rows: 25,
        };

        assert_eq!(result.sessions, 0);
        assert_eq!(result.tasks, 25);
    }
}
