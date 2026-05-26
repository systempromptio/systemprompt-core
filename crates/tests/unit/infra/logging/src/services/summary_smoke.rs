//! Tests for `ValidationSummary`, `OperationResult`, `ProgressSummary` -
//! builder semantics plus `Display` rendering smoke.

use systemprompt_logging::services::cli::{
    Display, OperationResult, ProgressSummary, ValidationSummary,
};

mod validation_summary {
    use super::*;

    #[test]
    fn default_is_empty() {
        let s = ValidationSummary::default();
        assert_eq!(s.total_active(), 0);
        assert!(!s.has_changes());
    }

    #[test]
    fn add_methods_grow_buckets() {
        let mut s = ValidationSummary::new();
        s.add_valid("a".into(), "1.0".into());
        s.add_installed("b".into());
        s.add_updated("c".into());
        s.add_schema_applied("d".into());
        s.add_seed_applied("e".into());
        s.add_disabled("f".into());
        assert_eq!(s.total_active(), 3);
        assert!(s.has_changes());
    }

    #[test]
    fn display_renders_all_paths() {
        let mut s = ValidationSummary::new();
        s.add_valid("a".into(), "1.0".into());
        s.add_installed("b".into());
        s.add_updated("c".into());
        s.add_schema_applied("d".into());
        s.add_seed_applied("e".into());
        s.add_disabled("f".into());
        s.display();
    }

    #[test]
    fn empty_display_does_not_panic() {
        ValidationSummary::new().display();
    }
}

mod operation_result {
    use super::*;

    #[test]
    fn success_constructor() {
        let r = OperationResult::success("install");
        assert!(r.success);
        assert!(r.message.is_none());
        r.display();
    }

    #[test]
    fn failure_constructor_with_message() {
        let r = OperationResult::failure("install", "permission denied");
        assert!(!r.success);
        assert_eq!(r.message.as_deref(), Some("permission denied"));
        r.display();
    }

    #[test]
    fn builders_chain() {
        let r = OperationResult::success("op")
            .with_message("msg")
            .with_detail("d1")
            .with_detail("d2");
        assert_eq!(r.message.as_deref(), Some("msg"));
        assert_eq!(r.details.len(), 2);
        r.display();
    }

    #[test]
    fn with_details_replaces_all() {
        let r = OperationResult::success("op")
            .with_detail("first")
            .with_details(vec!["a".into(), "b".into()]);
        assert_eq!(r.details, vec!["a".to_string(), "b".to_string()]);
    }
}

mod progress_summary {
    use super::*;

    #[test]
    fn new_initial_counts() {
        let p = ProgressSummary::new("download", 10);
        assert_eq!(p.total, 10);
        assert_eq!(p.completed, 0);
        assert_eq!(p.failed, 0);
        assert!(!p.is_complete());
    }

    #[test]
    fn add_success_and_failure() {
        let mut p = ProgressSummary::new("op", 4);
        p.add_success();
        p.add_success();
        p.add_failure();
        assert_eq!(p.completed, 2);
        assert_eq!(p.failed, 1);
        assert!(!p.is_complete());
        p.add_failure();
        assert!(p.is_complete());
    }

    #[test]
    fn success_rate_zero_total_is_one() {
        let p = ProgressSummary::new("op", 0);
        assert!((p.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn success_rate_half() {
        let mut p = ProgressSummary::new("op", 4);
        p.add_success();
        p.add_success();
        assert!((p.success_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn display_renders_all_branches() {
        // success path: failed == 0, total non-zero
        let mut p = ProgressSummary::new("ok-op", 2);
        p.add_success();
        p.add_success();
        p.display();

        // warning path: some completed, some failed
        let mut p = ProgressSummary::new("warn-op", 4);
        p.add_success();
        p.add_failure();
        p.display();

        // error path: only failures
        let mut p = ProgressSummary::new("err-op", 2);
        p.add_failure();
        p.add_failure();
        p.display();
    }
}
