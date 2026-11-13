use systemprompt_traits::Job;

mod ghost_session_cleanup_tests {
    use super::*;
    use systemprompt_analytics::jobs::GhostSessionCleanupJob;

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
}

mod malicious_ip_blacklist_tests {
    use super::*;
    use systemprompt_analytics::jobs::MaliciousIpBlacklistJob;

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
    use systemprompt_analytics::jobs::NoJsCleanupJob;

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
    use systemprompt_analytics::jobs::{
        BehavioralAnalysisJob, GhostSessionCleanupJob, MaliciousIpBlacklistJob, NoJsCleanupJob,
    };

    fn scheduled() -> Vec<&'static dyn Job> {
        vec![
            &BehavioralAnalysisJob,
            &GhostSessionCleanupJob,
            &MaliciousIpBlacklistJob,
            &NoJsCleanupJob,
        ]
    }

    #[test]
    fn scheduled_jobs_have_unique_names() {
        let names: Vec<&str> = scheduled().iter().map(|job| job.name()).collect();
        let unique: HashSet<&str> = names.iter().copied().collect();
        assert_eq!(names.len(), unique.len(), "duplicate job names: {names:?}");
    }

    #[test]
    fn scheduled_jobs_have_valid_cron_schedules() {
        for job in scheduled() {
            assert_eq!(
                job.schedule().split_whitespace().count(),
                6,
                "job '{}' has invalid cron schedule '{}'",
                job.name(),
                job.schedule()
            );
        }
    }

    #[test]
    fn scheduled_jobs_have_non_empty_descriptions() {
        for job in scheduled() {
            assert!(!job.description().is_empty(), "job '{}'", job.name());
        }
    }

    #[test]
    fn every_analytics_job_is_registered_in_inventory() {
        let registered: HashSet<&'static str> = inventory::iter::<&'static dyn Job>
            .into_iter()
            .map(|j| j.name())
            .collect();
        for name in [
            "backfill_session_geo",
            "behavioral_analysis",
            "ghost_session_cleanup",
            "malicious_ip_blacklist",
            "no_js_cleanup",
        ] {
            assert!(
                registered.contains(name),
                "job '{name}' not found in inventory"
            );
        }
    }
}

mod backfill_session_geo_tests {
    use super::*;
    use systemprompt_analytics::jobs::BackfillSessionGeoJob;

    #[test]
    fn name_is_backfill_session_geo() {
        assert_eq!(BackfillSessionGeoJob.name(), "backfill_session_geo");
    }

    #[test]
    fn schedule_is_empty_so_the_job_never_fires_on_a_timer() {
        assert_eq!(BackfillSessionGeoJob.schedule(), "");
    }

    #[test]
    fn is_not_schedulable() {
        assert!(
            !BackfillSessionGeoJob.schedulable(),
            "an on-demand backfill must not be registered with the cron scheduler"
        );
    }

    #[test]
    fn description_documents_the_batch_size_parameter() {
        let desc = BackfillSessionGeoJob.description();
        assert!(
            desc.contains("batch_size"),
            "operators discover job parameters from the description: {desc}"
        );
    }
}

mod behavioral_analysis_tests {
    use super::*;
    use systemprompt_analytics::jobs::BehavioralAnalysisJob;

    #[test]
    fn name_description_and_schedule_are_unchanged() {
        let job = BehavioralAnalysisJob;
        assert_eq!(job.name(), "behavioral_analysis");
        assert_eq!(
            job.description(),
            "Analyzes fingerprint behavior patterns and flags suspicious activity"
        );
        assert_eq!(job.schedule(), "0 0 * * * *");
    }
}
