use systemprompt_identifiers::UserId;
use systemprompt_scheduler::{JobConfig, SchedulerConfig};

mod job_config_tests {
    use super::*;

    #[test]
    fn new_defaults_owner_to_none() {
        let cfg = JobConfig::new("my_job");
        assert_eq!(cfg.name, "my_job");
        assert!(cfg.owner.is_none());
    }

    #[test]
    fn with_owner_sets_explicit_owner() {
        let owner = UserId::new("user-123");
        let cfg = JobConfig::new("my_job").with_owner(owner.clone());
        assert_eq!(cfg.owner, Some(owner));
    }

    #[test]
    fn new_is_enabled_by_default() {
        let cfg = JobConfig::new("job");
        assert!(cfg.enabled);
    }

    #[test]
    fn new_has_no_schedule_by_default() {
        let cfg = JobConfig::new("job");
        assert!(cfg.schedule.is_none());
    }

    #[test]
    fn new_has_no_extension_by_default() {
        let cfg = JobConfig::new("job");
        assert!(cfg.extension.is_none());
    }

    #[test]
    fn with_schedule_sets_schedule() {
        let cfg = JobConfig::new("job").with_schedule("0 0 * * * *");
        assert_eq!(cfg.schedule, Some("0 0 * * * *".to_string()));
    }

    #[test]
    fn with_extension_sets_extension() {
        let cfg = JobConfig::new("job").with_extension("core");
        assert_eq!(cfg.extension, Some("core".to_string()));
    }

    #[test]
    fn disabled_sets_enabled_false() {
        let cfg = JobConfig::new("job").disabled();
        assert!(!cfg.enabled);
    }

    #[test]
    fn new_defaults_enforce_to_false() {
        let cfg = JobConfig::new("job");
        assert!(!cfg.enforce);
    }

    #[test]
    fn with_enforce_opts_in() {
        let cfg = JobConfig::new("job").with_enforce();
        assert!(cfg.enforce);
    }

    #[test]
    fn deserialized_config_defaults_enforce_to_false() {
        let cfg: JobConfig =
            serde_json::from_str(r#"{"name": "behavioral_analysis", "schedule": "0 0 * * * *"}"#)
                .expect("valid job config");
        assert!(!cfg.enforce);
    }

    #[test]
    fn deserialized_config_honours_explicit_enforce() {
        let cfg: JobConfig =
            serde_json::from_str(r#"{"name": "malicious_ip_blacklist", "enforce": true}"#)
                .expect("valid job config");
        assert!(cfg.enforce);
    }

    #[test]
    fn builder_chain_works() {
        let owner = UserId::new("user-1");
        let cfg = JobConfig::new("complex_job")
            .with_owner(owner.clone())
            .with_extension("scheduler")
            .with_schedule("0 */5 * * * *")
            .disabled();
        assert_eq!(cfg.name, "complex_job");
        assert_eq!(cfg.owner, Some(owner));
        assert_eq!(cfg.extension, Some("scheduler".to_string()));
        assert_eq!(cfg.schedule, Some("0 */5 * * * *".to_string()));
        assert!(!cfg.enabled);
    }

    #[test]
    fn is_clone() {
        let cfg = JobConfig::new("clone_job").with_schedule("0 0 * * * *");
        let cloned = cfg.clone();
        assert_eq!(cloned.name, "clone_job");
        assert_eq!(cloned.schedule, Some("0 0 * * * *".to_string()));
    }

    #[test]
    fn is_debug() {
        let cfg = JobConfig::new("debug_job");
        let debug = format!("{:?}", cfg);
        assert!(debug.contains("debug_job"));
    }

    #[test]
    fn serializes_to_json() {
        let cfg = JobConfig::new("serde_job");
        let json = serde_json::to_string(&cfg).expect("JobConfig should serialize");
        assert!(json.contains("serde_job"));
    }

    #[test]
    fn name_accepts_long_string() {
        let long_name = "a".repeat(200);
        let cfg = JobConfig::new(long_name.clone());
        assert_eq!(cfg.name.len(), 200);
    }

    #[test]
    fn with_schedule_accepts_string_type() {
        let schedule = String::from("0 0 1 * * *");
        let cfg = JobConfig::new("job").with_schedule(schedule.clone());
        assert_eq!(cfg.schedule, Some(schedule));
    }
}

mod scheduler_config_tests {
    use super::*;

    #[test]
    fn with_system_admin_is_enabled() {
        let cfg = SchedulerConfig::with_system_admin();
        assert!(cfg.enabled);
    }

    #[test]
    fn with_system_admin_has_distributed_lock() {
        let cfg = SchedulerConfig::with_system_admin();
        assert!(cfg.distributed_lock);
    }

    #[test]
    fn with_system_admin_has_four_jobs() {
        let cfg = SchedulerConfig::with_system_admin();
        assert_eq!(
            cfg.jobs.len(),
            4,
            "expected 4 default jobs, got {}",
            cfg.jobs.len()
        );
    }

    #[test]
    fn with_system_admin_core_jobs_default_to_system_admin_owner() {
        let cfg = SchedulerConfig::with_system_admin();
        for job in &cfg.jobs {
            assert!(
                job.owner.is_none(),
                "core job '{}' should carry no explicit owner",
                job.name
            );
        }
    }

    #[test]
    fn with_system_admin_all_jobs_enabled() {
        let cfg = SchedulerConfig::with_system_admin();
        for job in &cfg.jobs {
            assert!(
                job.enabled,
                "job '{}' should be enabled by default",
                job.name
            );
        }
    }

    #[test]
    fn with_system_admin_all_jobs_have_extension_core() {
        let cfg = SchedulerConfig::with_system_admin();
        for job in &cfg.jobs {
            assert_eq!(
                job.extension.as_deref(),
                Some("core"),
                "job '{}' should have extension 'core'",
                job.name
            );
        }
    }

    #[test]
    fn with_system_admin_all_jobs_have_schedules() {
        let cfg = SchedulerConfig::with_system_admin();
        for job in &cfg.jobs {
            assert!(
                job.schedule.is_some(),
                "job '{}' should have an explicit schedule",
                job.name
            );
        }
    }

    #[test]
    fn with_system_admin_job_names_are_known() {
        let cfg = SchedulerConfig::with_system_admin();
        let expected = [
            "cleanup_anonymous_users",
            "cleanup_empty_contexts",
            "cleanup_inactive_sessions",
            "database_cleanup",
        ];
        let names: Vec<&str> = cfg.jobs.iter().map(|j| j.name.as_str()).collect();
        for expected_name in expected {
            assert!(
                names.contains(&expected_name),
                "expected job '{}' not found; got {:?}",
                expected_name,
                names
            );
        }
    }

    #[test]
    fn with_system_admin_bootstrap_jobs_are_subset_of_configured() {
        let cfg = SchedulerConfig::with_system_admin();
        let configured: std::collections::HashSet<&str> =
            cfg.jobs.iter().map(|j| j.name.as_str()).collect();
        for bootstrap in &cfg.bootstrap_jobs {
            assert!(
                configured.contains(bootstrap.as_str()),
                "bootstrap job '{}' has no matching JobConfig",
                bootstrap
            );
        }
    }

    #[test]
    fn is_clone() {
        let cfg = SchedulerConfig::with_system_admin();
        let cloned = cfg.clone();
        assert_eq!(cloned.jobs.len(), cfg.jobs.len());
        assert_eq!(cloned.enabled, cfg.enabled);
    }

    #[test]
    fn is_debug() {
        let cfg = SchedulerConfig::with_system_admin();
        let debug = format!("{:?}", cfg);
        assert!(debug.contains("SchedulerConfig"));
    }

    #[test]
    fn bootstrap_jobs_default_has_two_entries() {
        let cfg = SchedulerConfig::with_system_admin();
        assert_eq!(cfg.bootstrap_jobs.len(), 2);
    }

    #[test]
    fn bootstrap_jobs_contains_database_cleanup() {
        let cfg = SchedulerConfig::with_system_admin();
        assert!(
            cfg.bootstrap_jobs.contains(&"database_cleanup".to_string()),
            "bootstrap_jobs should include database_cleanup"
        );
    }

    #[test]
    fn bootstrap_jobs_contains_cleanup_inactive_sessions() {
        let cfg = SchedulerConfig::with_system_admin();
        assert!(
            cfg.bootstrap_jobs
                .contains(&"cleanup_inactive_sessions".to_string()),
            "bootstrap_jobs should include cleanup_inactive_sessions"
        );
    }

    #[test]
    fn all_job_schedules_are_6_part_cron() {
        let cfg = SchedulerConfig::with_system_admin();
        for job in &cfg.jobs {
            let schedule = job.schedule.as_deref().unwrap_or("");
            assert_eq!(
                schedule.split_whitespace().count(),
                6,
                "job '{}' has invalid schedule '{}'",
                job.name,
                schedule
            );
        }
    }
}
