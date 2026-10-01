//! Tests for ReconciliationResult

use systemprompt_identifiers::ServiceName;
use systemprompt_scheduler::ReconciliationResult;

mod new_tests {
    use super::*;

    #[test]
    fn new_has_empty_started() {
        let result = ReconciliationResult::new();
        assert!(result.started.is_empty());
    }

    #[test]
    fn new_has_empty_stopped() {
        let result = ReconciliationResult::new();
        assert!(result.stopped.is_empty());
    }

    #[test]
    fn new_has_empty_restarted() {
        let result = ReconciliationResult::new();
        assert!(result.restarted.is_empty());
    }

    #[test]
    fn new_has_empty_cleaned_up() {
        let result = ReconciliationResult::new();
        assert!(result.cleaned_up.is_empty());
    }

    #[test]
    fn new_has_empty_failed() {
        let result = ReconciliationResult::new();
        assert!(result.failed.is_empty());
    }

    #[test]
    fn default_is_same_as_new() {
        let new_result = ReconciliationResult::new();
        let default_result = ReconciliationResult::default();
        assert_eq!(new_result.started.len(), default_result.started.len());
        assert_eq!(new_result.stopped.len(), default_result.stopped.len());
        assert_eq!(new_result.restarted.len(), default_result.restarted.len());
        assert_eq!(new_result.cleaned_up.len(), default_result.cleaned_up.len());
        assert_eq!(new_result.failed.len(), default_result.failed.len());
    }
}

mod is_success_tests {
    use super::*;

    #[test]
    fn empty_result_is_success() {
        let result = ReconciliationResult::new();
        assert!(result.is_success());
    }

    #[test]
    fn result_with_started_is_success() {
        let mut result = ReconciliationResult::new();
        result.started.push(ServiceName::new("service1"));
        assert!(result.is_success());
    }

    #[test]
    fn result_with_stopped_is_success() {
        let mut result = ReconciliationResult::new();
        result.stopped.push(ServiceName::new("service1"));
        assert!(result.is_success());
    }

    #[test]
    fn result_with_restarted_is_success() {
        let mut result = ReconciliationResult::new();
        result.restarted.push(ServiceName::new("service1"));
        assert!(result.is_success());
    }

    #[test]
    fn result_with_cleaned_up_is_success() {
        let mut result = ReconciliationResult::new();
        result.cleaned_up.push(ServiceName::new("service1"));
        assert!(result.is_success());
    }

    #[test]
    fn result_with_failed_is_not_success() {
        let mut result = ReconciliationResult::new();
        result
            .failed
            .push((ServiceName::new("service1"), "error".to_string()));
        assert!(!result.is_success());
    }

    #[test]
    fn result_with_multiple_failures_is_not_success() {
        let mut result = ReconciliationResult::new();
        result
            .failed
            .push((ServiceName::new("service1"), "error1".to_string()));
        result
            .failed
            .push((ServiceName::new("service2"), "error2".to_string()));
        assert!(!result.is_success());
    }

    #[test]
    fn result_with_mixed_success_and_failure_is_not_success() {
        let mut result = ReconciliationResult::new();
        result.started.push(ServiceName::new("service1"));
        result.stopped.push(ServiceName::new("service2"));
        result
            .failed
            .push((ServiceName::new("service3"), "error".to_string()));
        assert!(!result.is_success());
    }
}

mod total_actions_tests {
    use super::*;

    #[test]
    fn empty_result_has_zero_actions() {
        let result = ReconciliationResult::new();
        assert_eq!(result.total_actions(), 0);
    }

    #[test]
    fn counts_started() {
        let mut result = ReconciliationResult::new();
        result.started.push(ServiceName::new("service1"));
        assert_eq!(result.total_actions(), 1);
    }

    #[test]
    fn counts_stopped() {
        let mut result = ReconciliationResult::new();
        result.stopped.push(ServiceName::new("service1"));
        assert_eq!(result.total_actions(), 1);
    }

    #[test]
    fn counts_restarted() {
        let mut result = ReconciliationResult::new();
        result.restarted.push(ServiceName::new("service1"));
        assert_eq!(result.total_actions(), 1);
    }

    #[test]
    fn counts_cleaned_up() {
        let mut result = ReconciliationResult::new();
        result.cleaned_up.push(ServiceName::new("service1"));
        assert_eq!(result.total_actions(), 1);
    }

    #[test]
    fn does_not_count_failed() {
        let mut result = ReconciliationResult::new();
        result
            .failed
            .push((ServiceName::new("service1"), "error".to_string()));
        assert_eq!(result.total_actions(), 0);
    }

    #[test]
    fn counts_all_action_types() {
        let mut result = ReconciliationResult::new();
        result.started.push(ServiceName::new("s1"));
        result.stopped.push(ServiceName::new("s2"));
        result.restarted.push(ServiceName::new("s3"));
        result.cleaned_up.push(ServiceName::new("s4"));
        assert_eq!(result.total_actions(), 4);
    }

    #[test]
    fn counts_multiple_per_type() {
        let mut result = ReconciliationResult::new();
        result.started.push(ServiceName::new("s1"));
        result.started.push(ServiceName::new("s2"));
        result.stopped.push(ServiceName::new("s3"));
        result.stopped.push(ServiceName::new("s4"));
        result.stopped.push(ServiceName::new("s5"));
        assert_eq!(result.total_actions(), 5);
    }

    #[test]
    fn counts_correctly_with_mixed_success_and_failure() {
        let mut result = ReconciliationResult::new();
        result.started.push(ServiceName::new("s1"));
        result.started.push(ServiceName::new("s2"));
        result
            .failed
            .push((ServiceName::new("s3"), "error".to_string()));
        result
            .failed
            .push((ServiceName::new("s4"), "error".to_string()));
        assert_eq!(result.total_actions(), 2);
    }
}

mod debug_tests {
    use super::*;


    #[test]
    fn debug_shows_started() {
        let mut result = ReconciliationResult::new();
        result.started.push(ServiceName::new("test-service"));
        let debug = format!("{:?}", result);
        assert!(debug.contains("test-service"));
    }
}
