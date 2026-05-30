use systemprompt_traits::Job;

mod ghost_session_cleanup_tests {
    use super::*;
    use systemprompt_scheduler::GhostSessionCleanupJob;

    #[test]
    fn name_is_ghost_session_cleanup() {
        assert_eq!(GhostSessionCleanupJob.name(), "ghost_session_cleanup");
    }

    #[test]
    fn description_is_non_empty() {
        assert!(!GhostSessionCleanupJob.description().is_empty());
    }

    #[test]
    fn description_mentions_ghost() {
        let desc = GhostSessionCleanupJob.description();
        let lower = desc.to_lowercase();
        assert!(
            lower.contains("ghost") || lower.contains("session"),
            "description should reference ghost sessions: {}",
            desc
        );
    }

    #[test]
    fn schedule_is_6_part_cron() {
        let schedule = GhostSessionCleanupJob.schedule();
        assert_eq!(
            schedule.split_whitespace().count(),
            6,
            "schedule '{}' should be 6-part cron",
            schedule
        );
    }

    #[test]
    fn name_is_snake_case() {
        let name = GhostSessionCleanupJob.name();
        assert!(
            name.chars().all(|c| c.is_lowercase() || c == '_'),
            "name '{}' should be snake_case",
            name
        );
    }

    #[test]
    fn is_copy() {
        let job1 = GhostSessionCleanupJob;
        let job2 = job1;
        assert_eq!(job1.name(), job2.name());
    }
}

mod malicious_ip_blacklist_tests {
    use super::*;
    use systemprompt_scheduler::MaliciousIpBlacklistJob;

    #[test]
    fn name_is_malicious_ip_blacklist() {
        assert_eq!(MaliciousIpBlacklistJob.name(), "malicious_ip_blacklist");
    }

    #[test]
    fn description_is_non_empty() {
        assert!(!MaliciousIpBlacklistJob.description().is_empty());
    }

    #[test]
    fn description_mentions_malicious_or_ip() {
        let desc = MaliciousIpBlacklistJob.description().to_lowercase();
        assert!(
            desc.contains("malicious") || desc.contains("ip") || desc.contains("blacklist"),
            "description should reference malicious IPs: {}",
            desc
        );
    }

    #[test]
    fn schedule_is_6_part_cron() {
        let schedule = MaliciousIpBlacklistJob.schedule();
        assert_eq!(
            schedule.split_whitespace().count(),
            6,
            "schedule '{}' should be 6-part cron",
            schedule
        );
    }

    #[test]
    fn name_is_snake_case() {
        let name = MaliciousIpBlacklistJob.name();
        assert!(
            name.chars().all(|c| c.is_lowercase() || c == '_'),
            "name '{}' should be snake_case",
            name
        );
    }

    #[test]
    fn is_copy() {
        let job1 = MaliciousIpBlacklistJob;
        let job2 = job1;
        assert_eq!(job1.name(), job2.name());
    }
}

mod no_js_cleanup_tests {
    use super::*;
    use systemprompt_scheduler::NoJsCleanupJob;

    #[test]
    fn name_is_no_js_cleanup() {
        assert_eq!(NoJsCleanupJob.name(), "no_js_cleanup");
    }

    #[test]
    fn description_is_non_empty() {
        assert!(!NoJsCleanupJob.description().is_empty());
    }

    #[test]
    fn description_mentions_javascript_or_js() {
        let desc = NoJsCleanupJob.description().to_lowercase();
        assert!(
            desc.contains("javascript") || desc.contains("js") || desc.contains("session"),
            "description should reference JS/javascript: {}",
            desc
        );
    }

    #[test]
    fn schedule_is_6_part_cron() {
        let schedule = NoJsCleanupJob.schedule();
        assert_eq!(
            schedule.split_whitespace().count(),
            6,
            "schedule '{}' should be 6-part cron",
            schedule
        );
    }

    #[test]
    fn name_is_snake_case() {
        let name = NoJsCleanupJob.name();
        assert!(
            name.chars().all(|c| c.is_lowercase() || c == '_'),
            "name '{}' should be snake_case",
            name
        );
    }

    #[test]
    fn is_copy() {
        let job1 = NoJsCleanupJob;
        let job2 = job1;
        assert_eq!(job1.name(), job2.name());
    }
}

mod all_jobs_inventory {
    use super::*;
    use std::collections::HashSet;
    use systemprompt_scheduler::{
        BehavioralAnalysisJob, CleanupEmptyContextsJob, CleanupInactiveSessionsJob,
        DatabaseCleanupJob, GhostSessionCleanupJob, MaliciousIpBlacklistJob, NoJsCleanupJob,
    };

    #[test]
    fn all_seven_jobs_have_unique_names() {
        let names: Vec<&str> = vec![
            BehavioralAnalysisJob.name(),
            CleanupEmptyContextsJob.name(),
            CleanupInactiveSessionsJob.name(),
            DatabaseCleanupJob.name(),
            GhostSessionCleanupJob.name(),
            MaliciousIpBlacklistJob.name(),
            NoJsCleanupJob.name(),
        ];
        let unique: HashSet<&str> = names.iter().copied().collect();
        assert_eq!(
            names.len(),
            unique.len(),
            "all job names must be unique; duplicates found in: {:?}",
            names
        );
    }

    #[test]
    fn all_seven_jobs_have_valid_cron_schedules() {
        let jobs: Vec<&dyn Job> = vec![
            &BehavioralAnalysisJob,
            &CleanupEmptyContextsJob,
            &CleanupInactiveSessionsJob,
            &DatabaseCleanupJob,
            &GhostSessionCleanupJob,
            &MaliciousIpBlacklistJob,
            &NoJsCleanupJob,
        ];
        for job in jobs {
            let schedule = job.schedule();
            assert_eq!(
                schedule.split_whitespace().count(),
                6,
                "job '{}' has invalid cron schedule '{}'",
                job.name(),
                schedule
            );
        }
    }

    #[test]
    fn all_seven_jobs_are_registered_in_inventory() {
        let registered: HashSet<&'static str> = inventory::iter::<&'static dyn Job>
            .into_iter()
            .map(|j| j.name())
            .collect();

        let expected = [
            "behavioral_analysis",
            "cleanup_empty_contexts",
            "cleanup_inactive_sessions",
            "database_cleanup",
            "ghost_session_cleanup",
            "malicious_ip_blacklist",
            "no_js_cleanup",
        ];

        for name in expected {
            assert!(
                registered.contains(name),
                "job '{}' not found in inventory",
                name
            );
        }
    }

    #[test]
    fn all_seven_jobs_have_non_empty_descriptions() {
        let jobs: Vec<&dyn Job> = vec![
            &BehavioralAnalysisJob,
            &CleanupEmptyContextsJob,
            &CleanupInactiveSessionsJob,
            &DatabaseCleanupJob,
            &GhostSessionCleanupJob,
            &MaliciousIpBlacklistJob,
            &NoJsCleanupJob,
        ];
        for job in jobs {
            assert!(
                !job.description().is_empty(),
                "job '{}' has empty description",
                job.name()
            );
        }
    }
}
