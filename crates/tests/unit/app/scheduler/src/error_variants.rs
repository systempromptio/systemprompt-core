use systemprompt_scheduler::SchedulerError;

mod additional_variants {
    use super::*;

    #[test]
    fn missing_context_message() {
        let err = SchedulerError::missing_context("DbPool");
        assert_eq!(err.to_string(), "Job context missing dependency: DbPool");
    }

    #[test]
    fn missing_context_accepts_string() {
        let err = SchedulerError::missing_context(String::from("AppContext"));
        assert_eq!(
            err.to_string(),
            "Job context missing dependency: AppContext"
        );
    }

    #[test]
    fn panic_message() {
        let err = SchedulerError::panic("out of memory");
        assert_eq!(err.to_string(), "Job panicked: out of memory");
    }

    #[test]
    fn panic_accepts_string() {
        let err = SchedulerError::panic(String::from("stack overflow"));
        assert_eq!(err.to_string(), "Job panicked: stack overflow");
    }

    #[test]
    fn distributed_lock_message() {
        let err = SchedulerError::DistributedLock("connection refused".to_string());
        assert_eq!(
            err.to_string(),
            "Distributed lock error: connection refused"
        );
    }

    #[test]
    fn internal_message() {
        let err = SchedulerError::Internal("unexpected state".to_string());
        assert_eq!(err.to_string(), "internal: unexpected state");
    }

    #[test]
    fn unresolved_job_owner_message() {
        let err = SchedulerError::UnresolvedJobOwner {
            job_name: "database_cleanup".to_string(),
            owner: "platform-admin".to_string(),
        };
        let msg = err.to_string();
        assert!(
            msg.contains("database_cleanup"),
            "error message should contain job name"
        );
        assert!(
            msg.contains("platform-admin"),
            "error message should contain owner name"
        );
    }

    #[test]
    fn unresolved_job_owner_mentions_active_user() {
        let err = SchedulerError::UnresolvedJobOwner {
            job_name: "my_job".to_string(),
            owner: "nobody".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("active"), "error should mention active user requirement");
    }

    #[test]
    fn io_error_from_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let scheduler_err = SchedulerError::from(io_err);
        assert!(scheduler_err.to_string().contains("I/O error"));
    }

    #[test]
    fn io_error_message_preserved() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let scheduler_err = SchedulerError::from(io_err);
        let msg = scheduler_err.to_string();
        assert!(msg.contains("access denied") || msg.contains("I/O"));
    }

    #[test]
    fn all_variants_are_debug() {
        let variants = [
            SchedulerError::missing_context("ctx"),
            SchedulerError::panic("boom"),
            SchedulerError::DistributedLock("lock err".to_string()),
            SchedulerError::Internal("internal".to_string()),
            SchedulerError::UnresolvedJobOwner {
                job_name: "j".to_string(),
                owner: "o".to_string(),
            },
            SchedulerError::AlreadyRunning,
            SchedulerError::NotInitialized,
        ];
        for err in variants {
            let debug = format!("{:?}", err);
            assert!(!debug.is_empty());
        }
    }

    #[test]
    fn missing_context_empty_string() {
        let err = SchedulerError::missing_context("");
        assert_eq!(err.to_string(), "Job context missing dependency: ");
    }

    #[test]
    fn internal_empty_string() {
        let err = SchedulerError::Internal(String::new());
        assert_eq!(err.to_string(), "internal: ");
    }

    #[test]
    fn distributed_lock_empty_string() {
        let err = SchedulerError::DistributedLock(String::new());
        assert_eq!(err.to_string(), "Distributed lock error: ");
    }
}
