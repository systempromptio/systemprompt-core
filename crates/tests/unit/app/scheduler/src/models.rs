//! Tests for scheduler models

use systemprompt_scheduler::{JobStatus, SchedulerError};

mod job_status_tests {
    use super::*;

    #[test]
    fn success_as_str_returns_lowercase() {
        assert_eq!(JobStatus::Success.as_str(), "success");
    }

    #[test]
    fn failed_as_str_returns_lowercase() {
        assert_eq!(JobStatus::Failed.as_str(), "failed");
    }

    #[test]
    fn running_as_str_returns_lowercase() {
        assert_eq!(JobStatus::Running.as_str(), "running");
    }

    #[test]
    fn display_matches_as_str_for_success() {
        assert_eq!(format!("{}", JobStatus::Success), "success");
    }

    #[test]
    fn display_matches_as_str_for_failed() {
        assert_eq!(format!("{}", JobStatus::Failed), "failed");
    }

    #[test]
    fn display_matches_as_str_for_running() {
        assert_eq!(format!("{}", JobStatus::Running), "running");
    }

    #[test]
    fn success_serializes_to_lowercase() {
        let json = serde_json::to_string(&JobStatus::Success).unwrap();
        assert_eq!(json, "\"success\"");
    }

    #[test]
    fn failed_serializes_to_lowercase() {
        let json = serde_json::to_string(&JobStatus::Failed).unwrap();
        assert_eq!(json, "\"failed\"");
    }

    #[test]
    fn running_serializes_to_lowercase() {
        let json = serde_json::to_string(&JobStatus::Running).unwrap();
        assert_eq!(json, "\"running\"");
    }

    #[test]
    fn deserializes_success_from_lowercase() {
        let status: JobStatus = serde_json::from_str("\"success\"").unwrap();
        assert_eq!(status, JobStatus::Success);
    }

    #[test]
    fn deserializes_failed_from_lowercase() {
        let status: JobStatus = serde_json::from_str("\"failed\"").unwrap();
        assert_eq!(status, JobStatus::Failed);
    }

    #[test]
    fn deserializes_running_from_lowercase() {
        let status: JobStatus = serde_json::from_str("\"running\"").unwrap();
        assert_eq!(status, JobStatus::Running);
    }

    #[test]
    fn status_is_clone() {
        let status = JobStatus::Success;
        let cloned = status;
        assert_eq!(status, cloned);
    }

    #[test]
    fn status_is_copy() {
        let status = JobStatus::Running;
        let copied: JobStatus = status;
        assert_eq!(status, copied);
    }

    #[test]
    fn status_is_eq() {
        assert_eq!(JobStatus::Success, JobStatus::Success);
        assert_ne!(JobStatus::Success, JobStatus::Failed);
    }

    #[test]
    fn status_is_debug() {
        let debug = format!("{:?}", JobStatus::Success);
        assert!(debug.contains("Success"));
    }
}

mod scheduler_error_tests {
    use super::*;

    #[test]
    fn job_not_found_contains_job_name() {
        let error = SchedulerError::job_not_found("test_job");
        let message = error.to_string();
        assert!(message.contains("test_job"));
        assert!(message.contains("not found"));
    }

    #[test]
    fn invalid_schedule_contains_schedule() {
        let error = SchedulerError::invalid_schedule("* * * *");
        let message = error.to_string();
        assert!(message.contains("* * * *"));
        assert!(message.contains("Invalid"));
    }

    #[test]
    fn job_execution_failed_contains_job_name_and_error() {
        let error = SchedulerError::job_execution_failed("my_job", "timeout");
        let message = error.to_string();
        assert!(message.contains("my_job"));
        assert!(message.contains("timeout"));
    }

    #[test]
    fn config_error_contains_message() {
        let error = SchedulerError::config_error("missing required field");
        let message = error.to_string();
        assert!(message.contains("missing required field"));
    }

    #[test]
    fn job_not_found_accepts_string() {
        let error = SchedulerError::job_not_found(String::from("dynamic_job"));
        assert!(error.to_string().contains("dynamic_job"));
    }

    #[test]
    fn invalid_schedule_accepts_string() {
        let error = SchedulerError::invalid_schedule(String::from("bad cron"));
        assert!(error.to_string().contains("bad cron"));
    }

    #[test]
    fn job_execution_failed_accepts_strings() {
        let error =
            SchedulerError::job_execution_failed(String::from("job1"), String::from("error msg"));
        let message = error.to_string();
        assert!(message.contains("job1"));
        assert!(message.contains("error msg"));
    }

    #[test]
    fn config_error_accepts_string() {
        let error = SchedulerError::config_error(String::from("config issue"));
        assert!(error.to_string().contains("config issue"));
    }

    #[test]
    fn errors_implement_std_error() {
        let error: Box<dyn std::error::Error> =
            Box::new(SchedulerError::job_not_found("test"));
        assert!(error.to_string().contains("test"));
    }

    #[test]
    fn errors_are_debug() {
        let error = SchedulerError::job_not_found("debug_test");
        let debug = format!("{:?}", error);
        assert!(debug.contains("JobNotFound"));
    }
}
