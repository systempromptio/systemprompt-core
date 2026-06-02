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
        assert!(
            msg.contains("active"),
            "error should mention active user requirement"
        );
    }

    #[test]
    fn unknown_job_lists_offending_names() {
        let err = SchedulerError::UnknownJob {
            names: "governance_bootstrap, typo_job".to_string(),
        };
        let msg = err.to_string();
        assert!(
            msg.contains("governance_bootstrap") && msg.contains("typo_job"),
            "UnknownJob message must list every offending name"
        );
        assert!(
            msg.contains("submit_job!"),
            "UnknownJob message should point at the inventory registration macro"
        );
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

mod error_source_chain {
    use super::*;
    use std::error::Error;

    #[test]
    fn io_variant_exposes_underlying_source() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err = SchedulerError::from(io_err);
        let source = err.source().expect("Io variant must expose its source");
        assert!(source.to_string().contains("missing"));
    }

    #[test]
    fn string_carve_out_variants_have_no_source() {
        // The stringified carve-outs wrap no typed cause, so source() is None.
        assert!(SchedulerError::Internal("x".to_string()).source().is_none());
        assert!(
            SchedulerError::DistributedLock("x".to_string())
                .source()
                .is_none()
        );
        assert!(SchedulerError::panic("x").source().is_none());
        assert!(SchedulerError::missing_context("x").source().is_none());
    }

    #[test]
    fn structured_variants_have_no_source() {
        assert!(SchedulerError::AlreadyRunning.source().is_none());
        assert!(SchedulerError::NotInitialized.source().is_none());
        assert!(SchedulerError::job_not_found("j").source().is_none());
        assert!(SchedulerError::invalid_schedule("s").source().is_none());
        assert!(
            SchedulerError::job_execution_failed("j", "e")
                .source()
                .is_none()
        );
        assert!(SchedulerError::config_error("c").source().is_none());
    }
}

mod error_constructor_round_trips {
    use super::*;

    #[test]
    fn job_not_found_constructor_matches_struct_variant() {
        let via_ctor = SchedulerError::job_not_found("j").to_string();
        let via_struct = SchedulerError::JobNotFound {
            job_name: "j".to_string(),
        }
        .to_string();
        assert_eq!(via_ctor, via_struct);
    }

    #[test]
    fn invalid_schedule_constructor_matches_struct_variant() {
        let via_ctor = SchedulerError::invalid_schedule("0 0 *").to_string();
        let via_struct = SchedulerError::InvalidSchedule {
            schedule: "0 0 *".to_string(),
        }
        .to_string();
        assert_eq!(via_ctor, via_struct);
    }

    #[test]
    fn job_execution_failed_constructor_matches_struct_variant() {
        let via_ctor = SchedulerError::job_execution_failed("j", "boom").to_string();
        let via_struct = SchedulerError::JobExecutionFailed {
            job_name: "j".to_string(),
            error: "boom".to_string(),
        }
        .to_string();
        assert_eq!(via_ctor, via_struct);
    }

    #[test]
    fn config_error_constructor_matches_struct_variant() {
        let via_ctor = SchedulerError::config_error("bad").to_string();
        let via_struct = SchedulerError::ConfigError {
            message: "bad".to_string(),
        }
        .to_string();
        assert_eq!(via_ctor, via_struct);
    }

    #[test]
    fn missing_context_constructor_matches_struct_variant() {
        let via_ctor = SchedulerError::missing_context("DbPool").to_string();
        let via_struct = SchedulerError::MissingContext("DbPool".to_string()).to_string();
        assert_eq!(via_ctor, via_struct);
    }

    #[test]
    fn panic_constructor_matches_struct_variant() {
        let via_ctor = SchedulerError::panic("kaboom").to_string();
        let via_struct = SchedulerError::Panic("kaboom".to_string()).to_string();
        assert_eq!(via_ctor, via_struct);
    }
}
