use systemprompt_identifiers::UserId;
use systemprompt_scheduler::{JobConfig, SchedulerConfig};
use systemprompt_test_fixtures::fixture_system_admin;

mod job_config_tests {
    use super::*;

    #[test]
    fn new_sets_name_and_owner() {
        let owner = UserId::new("user-123");
        let cfg = JobConfig::new("my_job", owner.clone());
        assert_eq!(cfg.name, "my_job");
        assert_eq!(cfg.owner, owner);
    }

    #[test]
    fn new_is_enabled_by_default() {
        let owner = UserId::new("user-1");
        let cfg = JobConfig::new("job", owner);
        assert!(cfg.enabled);
    }

    #[test]
    fn new_has_no_schedule_by_default() {
        let owner = UserId::new("user-1");
        let cfg = JobConfig::new("job", owner);
        assert!(cfg.schedule.is_none());
    }

    #[test]
    fn new_has_no_extension_by_default() {
        let owner = UserId::new("user-1");
        let cfg = JobConfig::new("job", owner);
        assert!(cfg.extension.is_none());
    }

    #[test]
    fn with_schedule_sets_schedule() {
        let owner = UserId::new("user-1");
        let cfg = JobConfig::new("job", owner).with_schedule("0 0 * * * *");
        assert_eq!(cfg.schedule, Some("0 0 * * * *".to_string()));
    }

    #[test]
    fn with_extension_sets_extension() {
        let owner = UserId::new("user-1");
        let cfg = JobConfig::new("job", owner).with_extension("core");
        assert_eq!(cfg.extension, Some("core".to_string()));
    }

    #[test]
    fn disabled_sets_enabled_false() {
        let owner = UserId::new("user-1");
        let cfg = JobConfig::new("job", owner).disabled();
        assert!(!cfg.enabled);
    }

    #[test]
    fn builder_chain_works() {
        let owner = UserId::new("user-1");
        let cfg = JobConfig::new("complex_job", owner.clone())
            .with_extension("scheduler")
            .with_schedule("0 */5 * * * *")
            .disabled();
        assert_eq!(cfg.name, "complex_job");
        assert_eq!(cfg.owner, owner);
        assert_eq!(cfg.extension, Some("scheduler".to_string()));
        assert_eq!(cfg.schedule, Some("0 */5 * * * *".to_string()));
        assert!(!cfg.enabled);
    }

    #[test]
    fn is_clone() {
        let owner = UserId::new("user-1");
        let cfg = JobConfig::new("clone_job", owner).with_schedule("0 0 * * * *");
        let cloned = cfg.clone();
        assert_eq!(cloned.name, "clone_job");
        assert_eq!(cloned.schedule, Some("0 0 * * * *".to_string()));
    }

    #[test]
    fn is_debug() {
        let owner = UserId::new("user-1");
        let cfg = JobConfig::new("debug_job", owner);
        let debug = format!("{:?}", cfg);
        assert!(debug.contains("debug_job"));
    }

    #[test]
    fn serializes_to_json() {
        let owner = UserId::new("user-1");
        let cfg = JobConfig::new("serde_job", owner);
        let json = serde_json::to_string(&cfg).expect("JobConfig should serialize");
        assert!(json.contains("serde_job"));
    }

    #[test]
    fn name_accepts_long_string() {
        let owner = UserId::new("user-1");
        let long_name = "a".repeat(200);
        let cfg = JobConfig::new(long_name.clone(), owner);
        assert_eq!(cfg.name.len(), 200);
    }

    #[test]
    fn with_schedule_accepts_string_type() {
        let owner = UserId::new("user-1");
        let schedule = String::from("0 0 1 * * *");
        let cfg = JobConfig::new("job", owner).with_schedule(schedule.clone());
        assert_eq!(cfg.schedule, Some(schedule));
    }
}

mod scheduler_config_tests {
    use super::*;

    #[test]
    fn with_system_admin_is_enabled() {
        let admin = fixture_system_admin("platform-admin");
        let cfg = SchedulerConfig::with_system_admin(&admin);
        assert!(cfg.enabled);
    }

    #[test]
    fn with_system_admin_has_distributed_lock() {
        let admin = fixture_system_admin("platform-admin");
        let cfg = SchedulerConfig::with_system_admin(&admin);
        assert!(cfg.distributed_lock);
    }

    #[test]
    fn with_system_admin_has_four_jobs() {
        let admin = fixture_system_admin("platform-admin");
        let cfg = SchedulerConfig::with_system_admin(&admin);
        assert_eq!(
            cfg.jobs.len(),
            4,
            "expected 4 default jobs, got {}",
            cfg.jobs.len()
        );
    }

    #[test]
    fn with_system_admin_all_jobs_enabled() {
        let admin = fixture_system_admin("platform-admin");
        let cfg = SchedulerConfig::with_system_admin(&admin);
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
        let admin = fixture_system_admin("platform-admin");
        let cfg = SchedulerConfig::with_system_admin(&admin);
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
        let admin = fixture_system_admin("platform-admin");
        let cfg = SchedulerConfig::with_system_admin(&admin);
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
        let admin = fixture_system_admin("platform-admin");
        let cfg = SchedulerConfig::with_system_admin(&admin);
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
        let admin = fixture_system_admin("platform-admin");
        let cfg = SchedulerConfig::with_system_admin(&admin);
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
        let admin = fixture_system_admin("platform-admin");
        let cfg = SchedulerConfig::with_system_admin(&admin);
        let cloned = cfg.clone();
        assert_eq!(cloned.jobs.len(), cfg.jobs.len());
        assert_eq!(cloned.enabled, cfg.enabled);
    }

    #[test]
    fn is_debug() {
        let admin = fixture_system_admin("platform-admin");
        let cfg = SchedulerConfig::with_system_admin(&admin);
        let debug = format!("{:?}", cfg);
        assert!(debug.contains("SchedulerConfig"));
    }

    #[test]
    fn bootstrap_jobs_default_has_two_entries() {
        let admin = fixture_system_admin("platform-admin");
        let cfg = SchedulerConfig::with_system_admin(&admin);
        assert_eq!(cfg.bootstrap_jobs.len(), 2);
    }

    #[test]
    fn bootstrap_jobs_contains_database_cleanup() {
        let admin = fixture_system_admin("platform-admin");
        let cfg = SchedulerConfig::with_system_admin(&admin);
        assert!(
            cfg.bootstrap_jobs.contains(&"database_cleanup".to_string()),
            "bootstrap_jobs should include database_cleanup"
        );
    }

    #[test]
    fn bootstrap_jobs_contains_cleanup_inactive_sessions() {
        let admin = fixture_system_admin("platform-admin");
        let cfg = SchedulerConfig::with_system_admin(&admin);
        assert!(
            cfg.bootstrap_jobs
                .contains(&"cleanup_inactive_sessions".to_string()),
            "bootstrap_jobs should include cleanup_inactive_sessions"
        );
    }

    #[test]
    fn all_job_schedules_are_6_part_cron() {
        let admin = fixture_system_admin("platform-admin");
        let cfg = SchedulerConfig::with_system_admin(&admin);
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
